//! Billing routes (Stripe integration)

use crate::db::queries;
use crate::routes::AppState;
use axum::{
    body::Bytes,
    extract::{State, Request},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};

/// Stripe API base URL
const STRIPE_API_URL: &str = "https://api.stripe.com/v1";

/// Build the billing router
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/billing/checkout", post(create_checkout))
        .route("/api/billing/portal", get(customer_portal))
        .route("/api/billing/webhook", post(stripe_webhook))
        .route("/api/billing/plans", get(list_plans))
}

/// Request body for creating checkout session
#[derive(Debug, Deserialize)]
struct CreateCheckoutRequest {
    plan: String, // "hobby" or "pro"
}

/// Response for checkout session
#[derive(Debug, Serialize)]
struct CheckoutResponse {
    checkout_url: String,
}

/// Create a Stripe checkout session
async fn create_checkout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateCheckoutRequest>,
) -> Response {
    // Get user from token
    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, "Missing authorization").into_response(),
    };

    let user = match queries::find_user_by_token(&state.db, token).await {
        Ok(Some(user)) => user,
        Ok(None) => return (StatusCode::UNAUTHORIZED, "Invalid token").into_response(),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    // Get Stripe secret key
    let stripe_key = match std::env::var("STRIPE_SECRET_KEY") {
        Ok(key) => key,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, "Stripe not configured").into_response(),
    };

    // Validate plan and get price ID
    let price_id = match payload.plan.as_str() {
        "hobby" => std::env::var("STRIPE_HOBBY_PRICE_ID").unwrap_or_else(|_| "".to_string()),
        "pro" => std::env::var("STRIPE_PRO_PRICE_ID").unwrap_or_else(|_| "".to_string()),
        _ => return (StatusCode::BAD_REQUEST, "Invalid plan").into_response(),
    };

    if price_id.is_empty() {
        return (StatusCode::SERVICE_UNAVAILABLE, "Plan not configured").into_response();
    }

    // Create or get Stripe customer
    let customer_id = match &user.stripe_customer_id {
        Some(id) => id.clone(),
        None => {
            match create_stripe_customer(&state.http_client, &stripe_key, &user.email).await {
                Ok(id) => {
                    // Save customer ID
                    if let Err(e) = queries::update_stripe_customer(&state.db, user.id, &id).await {
                        tracing::error!("Failed to save Stripe customer ID: {}", e);
                    }
                    id
                }
                Err(e) => {
                    tracing::error!("Failed to create Stripe customer: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create customer").into_response();
                }
            }
        }
    };

    // Create checkout session
    let success_url = format!("{}/billing/success?session_id={{CHECKOUT_SESSION_ID}}", state.config.public_url);
    let cancel_url = format!("{}/billing/cancel", state.config.public_url);

    match create_checkout_session(&state.http_client, &stripe_key, &customer_id, &price_id, &success_url, &cancel_url).await {
        Ok(url) => Json(CheckoutResponse { checkout_url: url }).into_response(),
        Err(e) => {
            tracing::error!("Failed to create checkout session: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create checkout").into_response()
        }
    }
}

/// Get customer portal URL for managing subscription
async fn customer_portal(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, "Missing authorization").into_response(),
    };

    let user = match queries::find_user_by_token(&state.db, token).await {
        Ok(Some(user)) => user,
        Ok(None) => return (StatusCode::UNAUTHORIZED, "Invalid token").into_response(),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    let stripe_key = match std::env::var("STRIPE_SECRET_KEY") {
        Ok(key) => key,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, "Stripe not configured").into_response(),
    };

    let customer_id = match &user.stripe_customer_id {
        Some(id) => id,
        None => return (StatusCode::BAD_REQUEST, "No billing account").into_response(),
    };

    let return_url = format!("{}/dashboard", state.config.public_url);

    match create_portal_session(&state.http_client, &stripe_key, customer_id, &return_url).await {
        Ok(url) => Json(serde_json::json!({ "portal_url": url })).into_response(),
        Err(e) => {
            tracing::error!("Failed to create portal session: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create portal").into_response()
        }
    }
}

/// Stripe webhook handler
async fn stripe_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let webhook_secret = match std::env::var("STRIPE_WEBHOOK_SECRET") {
        Ok(secret) => secret,
        Err(_) => {
            tracing::warn!("STRIPE_WEBHOOK_SECRET not set");
            return (StatusCode::SERVICE_UNAVAILABLE, "Webhook not configured").into_response();
        }
    };

    // Verify webhook signature
    let signature = match headers.get("stripe-signature").and_then(|v| v.to_str().ok()) {
        Some(sig) => sig,
        None => return (StatusCode::BAD_REQUEST, "Missing signature").into_response(),
    };

    // Parse the event
    let payload_str = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid payload").into_response(),
    };

    // Verify signature (simplified - in production use stripe crate)
    if !verify_webhook_signature(payload_str, signature, &webhook_secret) {
        return (StatusCode::BAD_REQUEST, "Invalid signature").into_response();
    }

    let event: serde_json::Value = match serde_json::from_str(payload_str) {
        Ok(e) => e,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid JSON").into_response(),
    };

    let event_type = event["type"].as_str().unwrap_or("");
    tracing::info!("Received Stripe webhook: {}", event_type);

    match event_type {
        "checkout.session.completed" => {
            handle_checkout_completed(&state, &event["data"]["object"]).await;
        }
        "customer.subscription.updated" => {
            handle_subscription_updated(&state, &event["data"]["object"]).await;
        }
        "customer.subscription.deleted" => {
            handle_subscription_deleted(&state, &event["data"]["object"]).await;
        }
        "invoice.payment_failed" => {
            handle_payment_failed(&state, &event["data"]["object"]).await;
        }
        _ => {
            tracing::debug!("Unhandled webhook event: {}", event_type);
        }
    }

    (StatusCode::OK, "OK").into_response()
}

/// List available plans
async fn list_plans() -> Response {
    Json(serde_json::json!({
        "plans": [
            {
                "id": "free",
                "name": "Free",
                "price": 0,
                "features": {
                    "concurrent_tunnels": 5,
                    "tunnels_per_hour": 60,
                    "requests_per_min": 300,
                    "bandwidth_gb": 1,
                    "custom_domains": false,
                    "reserved_subdomains": false
                }
            },
            {
                "id": "hobby",
                "name": "Hobby",
                "price": 5,
                "features": {
                    "concurrent_tunnels": 10,
                    "tunnels_per_hour": 200,
                    "requests_per_min": 1000,
                    "bandwidth_gb": 50,
                    "custom_domains": true,
                    "reserved_subdomains": true
                }
            },
            {
                "id": "pro",
                "name": "Pro",
                "price": 15,
                "features": {
                    "concurrent_tunnels": 50,
                    "tunnels_per_hour": 1000,
                    "requests_per_min": 5000,
                    "bandwidth_gb": 500,
                    "custom_domains": true,
                    "reserved_subdomains": true,
                    "team_members": 5
                }
            }
        ]
    }))
    .into_response()
}

// Stripe API helpers

async fn create_stripe_customer(client: &reqwest::Client, api_key: &str, email: &str) -> Result<String, reqwest::Error> {
    // Use shared HTTP client for connection pooling
    let response: serde_json::Value = client
        .post(format!("{}/customers", STRIPE_API_URL))
        .basic_auth(api_key, None::<&str>)
        .form(&[("email", email)])
        .send()
        .await?
        .json()
        .await?;

    Ok(response["id"].as_str().unwrap_or("").to_string())
}

async fn create_checkout_session(
    client: &reqwest::Client,
    api_key: &str,
    customer_id: &str,
    price_id: &str,
    success_url: &str,
    cancel_url: &str,
) -> Result<String, reqwest::Error> {
    // Use shared HTTP client for connection pooling
    let response: serde_json::Value = client
        .post(format!("{}/checkout/sessions", STRIPE_API_URL))
        .basic_auth(api_key, None::<&str>)
        .form(&[
            ("customer", customer_id),
            ("mode", "subscription"),
            ("line_items[0][price]", price_id),
            ("line_items[0][quantity]", "1"),
            ("success_url", success_url),
            ("cancel_url", cancel_url),
        ])
        .send()
        .await?
        .json()
        .await?;

    Ok(response["url"].as_str().unwrap_or("").to_string())
}

async fn create_portal_session(
    client: &reqwest::Client,
    api_key: &str,
    customer_id: &str,
    return_url: &str,
) -> Result<String, reqwest::Error> {
    // Use shared HTTP client for connection pooling
    let response: serde_json::Value = client
        .post(format!("{}/billing_portal/sessions", STRIPE_API_URL))
        .basic_auth(api_key, None::<&str>)
        .form(&[
            ("customer", customer_id),
            ("return_url", return_url),
        ])
        .send()
        .await?
        .json()
        .await?;

    Ok(response["url"].as_str().unwrap_or("").to_string())
}

// Webhook handlers

async fn handle_checkout_completed(state: &AppState, session: &serde_json::Value) {
    let customer_id = session["customer"].as_str().unwrap_or("");
    let subscription_id = session["subscription"].as_str().unwrap_or("");

    if customer_id.is_empty() {
        tracing::warn!("Checkout completed but no customer ID");
        return;
    }

    // Find user by customer ID
    let user = match queries::find_user_by_stripe_customer(&state.db, customer_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::warn!("No user found for Stripe customer: {}", customer_id);
            return;
        }
        Err(e) => {
            tracing::error!("Database error: {}", e);
            return;
        }
    };

    // Get subscription details to determine plan
    let plan = determine_plan_from_subscription(&state.http_client, subscription_id).await.unwrap_or("free".to_string());

    // Update user's plan
    let expires_at = Utc::now() + Duration::days(30);
    if let Err(e) = queries::update_user_subscription(
        &state.db,
        user.id,
        &plan,
        Some(subscription_id),
        Some(expires_at),
    ).await {
        tracing::error!("Failed to update user subscription: {}", e);
    } else {
        tracing::info!("User {} upgraded to {} plan", user.email, plan);
    }
}

async fn handle_subscription_updated(state: &AppState, subscription: &serde_json::Value) {
    let customer_id = subscription["customer"].as_str().unwrap_or("");
    let subscription_id = subscription["id"].as_str().unwrap_or("");
    let status = subscription["status"].as_str().unwrap_or("");

    let user = match queries::find_user_by_stripe_customer(&state.db, customer_id).await {
        Ok(Some(user)) => user,
        _ => return,
    };

    let (plan, expires_at) = match status {
        "active" | "trialing" => {
            let plan = determine_plan_from_subscription(&state.http_client, subscription_id).await.unwrap_or("free".to_string());
            let period_end = subscription["current_period_end"].as_i64().unwrap_or(0);
            let expires = DateTime::from_timestamp(period_end, 0).unwrap_or(Utc::now());
            (plan, Some(expires))
        }
        "past_due" | "unpaid" => {
            // Give grace period - keep current plan
            let plan = determine_plan_from_subscription(&state.http_client, subscription_id).await.unwrap_or("free".to_string());
            (plan, Some(Utc::now() + Duration::days(7)))
        }
        "canceled" | "incomplete_expired" => {
            ("free".to_string(), None)
        }
        _ => return,
    };

    if let Err(e) = queries::update_user_subscription(
        &state.db,
        user.id,
        &plan,
        Some(subscription_id),
        expires_at,
    ).await {
        tracing::error!("Failed to update subscription: {}", e);
    }
}

async fn handle_subscription_deleted(state: &AppState, subscription: &serde_json::Value) {
    let customer_id = subscription["customer"].as_str().unwrap_or("");

    let user = match queries::find_user_by_stripe_customer(&state.db, customer_id).await {
        Ok(Some(user)) => user,
        _ => return,
    };

    // Downgrade to free
    if let Err(e) = queries::update_user_subscription(&state.db, user.id, "free", None, None).await {
        tracing::error!("Failed to downgrade user: {}", e);
    } else {
        tracing::info!("User {} downgraded to free plan", user.email);
    }
}

async fn handle_payment_failed(state: &AppState, invoice: &serde_json::Value) {
    let customer_id = invoice["customer"].as_str().unwrap_or("");

    // Log the failure - could send email notification here
    tracing::warn!("Payment failed for customer: {}", customer_id);
}

async fn determine_plan_from_subscription(http_client: &reqwest::Client, subscription_id: &str) -> Option<String> {
    let stripe_key = std::env::var("STRIPE_SECRET_KEY").ok()?;
    let hobby_price_id = std::env::var("STRIPE_HOBBY_PRICE_ID").unwrap_or_default();
    let pro_price_id = std::env::var("STRIPE_PRO_PRICE_ID").unwrap_or_default();

    // Fetch subscription from Stripe API using shared client
    let response: serde_json::Value = match http_client
        .get(format!("{}/subscriptions/{}", STRIPE_API_URL, subscription_id))
        .basic_auth(&stripe_key, None::<&str>)
        .send()
        .await
    {
        Ok(resp) => match resp.json().await {
            Ok(json) => json,
            Err(e) => {
                // SECURITY: Default to free on parse errors - don't grant paid access on failures
                tracing::error!("Failed to parse Stripe subscription response: {}", e);
                return Some("free".to_string());
            }
        },
        Err(e) => {
            // SECURITY: Default to free on API errors - don't grant paid access on failures
            tracing::error!("Failed to fetch Stripe subscription: {}", e);
            return Some("free".to_string());
        }
    };

    // Extract price ID from the subscription's first item
    let price_id = response["items"]["data"][0]["price"]["id"]
        .as_str()
        .unwrap_or("");

    // Determine plan based on price ID
    if !pro_price_id.is_empty() && price_id == pro_price_id {
        Some("pro".to_string())
    } else if !hobby_price_id.is_empty() && price_id == hobby_price_id {
        Some("hobby".to_string())
    } else {
        // SECURITY: Unknown price ID should not grant paid access
        tracing::warn!("Unknown price ID: {}, defaulting to free", price_id);
        Some("free".to_string())
    }
}

/// Maximum age of webhook timestamp to accept (5 minutes)
const WEBHOOK_TIMESTAMP_TOLERANCE_SECS: i64 = 300;

fn verify_webhook_signature(payload: &str, signature: &str, secret: &str) -> bool {
    // Parse signature header
    let parts: std::collections::HashMap<&str, &str> = signature
        .split(',')
        .filter_map(|part| {
            let mut iter = part.splitn(2, '=');
            Some((iter.next()?, iter.next()?))
        })
        .collect();

    let timestamp = match parts.get("t") {
        Some(t) => *t,
        None => return false,
    };

    // Verify timestamp is within tolerance (prevents replay attacks)
    let timestamp_secs: i64 = match timestamp.parse() {
        Ok(t) => t,
        Err(_) => {
            tracing::warn!("Invalid webhook timestamp format");
            return false;
        }
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let age = now - timestamp_secs;
    if age < 0 || age > WEBHOOK_TIMESTAMP_TOLERANCE_SECS {
        tracing::warn!("Webhook timestamp outside tolerance: {} seconds old", age);
        return false;
    }

    let provided_sig = match parts.get("v1") {
        Some(s) => *s,
        None => return false,
    };

    // Compute expected signature
    let signed_payload = format!("{}.{}", timestamp, payload);

    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };

    mac.update(signed_payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    // Constant-time comparison using subtle crate pattern
    // Compare byte-by-byte to avoid timing attacks
    if expected.len() != provided_sig.len() {
        return false;
    }

    let mut result = 0u8;
    for (a, b) in expected.bytes().zip(provided_sig.bytes()) {
        result |= a ^ b;
    }
    result == 0
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
}
