//! Authentication routes (GitHub OAuth)

use crate::db::queries;
use crate::routes::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use dvaar_common::constants;
use serde::Deserialize;
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};

/// OAuth state data stored in Redis
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct OAuthState {
    nonce: String,
    redirect_uri: Option<String>,
    created_at: u64,
}

/// OAuth state TTL in seconds (10 minutes)
const OAUTH_STATE_TTL: u64 = 600;

/// Build the auth router
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/github", get(github_redirect))
        .route("/api/auth/github/callback", get(github_callback))
        .route("/api/auth/cli", get(cli_auth))
        .route("/api/auth/config", get(auth_config))
        .route("/api/auth/token", post(exchange_token))
        .route("/api/user", get(get_user))
        .route("/api/usage", get(get_usage))
        .route("/api/nodes", get(get_nodes))
}

/// Query params for GitHub redirect
#[derive(Debug, Deserialize)]
pub struct GithubRedirectQuery {
    redirect_uri: Option<String>,
}

/// Redirect to GitHub OAuth
async fn github_redirect(
    State(state): State<AppState>,
    Query(query): Query<GithubRedirectQuery>,
) -> Response {
    let client_id = &state.config.github_client_id;

    if client_id.is_empty() {
        return (StatusCode::SERVICE_UNAVAILABLE, "GitHub OAuth not configured").into_response();
    }

    // Generate cryptographically secure nonce for CSRF protection
    let nonce = Uuid::new_v4().to_string();

    // Validate redirect_uri if provided (must be localhost or our domain)
    let redirect_uri = query.redirect_uri.and_then(|uri| {
        if uri.starts_with("http://localhost:") || uri.starts_with("http://127.0.0.1:") {
            Some(uri)
        } else {
            tracing::warn!("Rejected invalid redirect_uri: {}", uri);
            None
        }
    });

    let oauth_state = OAuthState {
        nonce: nonce.clone(),
        redirect_uri,
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };

    // Store state in Redis with TTL
    let state_key = format!("oauth_state:{}", nonce);
    let state_json = match serde_json::to_string(&oauth_state) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("Failed to serialize OAuth state: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    if let Err(e) = state.route_manager.store_oauth_state(&state_key, &state_json).await {
        tracing::error!("Failed to store OAuth state in Redis: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
    }

    let redirect_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&scope=user:email&state={}",
        client_id, nonce
    );

    Redirect::temporary(&redirect_url).into_response()
}

/// Query params for GitHub callback
#[derive(Debug, Deserialize)]
pub struct GithubCallbackQuery {
    code: String,
    state: Option<String>,
}

/// GitHub OAuth callback
async fn github_callback(
    State(state): State<AppState>,
    Query(query): Query<GithubCallbackQuery>,
) -> Response {
    // Verify OAuth state (CSRF protection)
    let oauth_state = match &query.state {
        Some(nonce) if !nonce.is_empty() => {
            let state_key = format!("oauth_state:{}", nonce);
            match state.route_manager.get_and_delete_oauth_state(&state_key).await {
                Ok(Some(state_json)) => {
                    match serde_json::from_str::<OAuthState>(&state_json) {
                        Ok(s) => {
                            // Verify state hasn't expired (double-check beyond Redis TTL)
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            if now - s.created_at > OAUTH_STATE_TTL {
                                tracing::warn!("OAuth state expired for nonce: {}", nonce);
                                return (StatusCode::BAD_REQUEST, "OAuth state expired").into_response();
                            }
                            s
                        }
                        Err(e) => {
                            tracing::error!("Failed to parse OAuth state: {}", e);
                            return (StatusCode::BAD_REQUEST, "Invalid OAuth state").into_response();
                        }
                    }
                }
                Ok(None) => {
                    tracing::warn!("OAuth state not found (possible replay): {}", nonce);
                    return (StatusCode::BAD_REQUEST, "Invalid or expired OAuth state").into_response();
                }
                Err(e) => {
                    tracing::error!("Failed to retrieve OAuth state: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
                }
            }
        }
        _ => {
            tracing::warn!("Missing OAuth state parameter");
            return (StatusCode::BAD_REQUEST, "Missing state parameter").into_response();
        }
    };

    // Exchange code for access token
    let token_response = match exchange_github_code(&state, &query.code).await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::error!("Failed to exchange GitHub code: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "OAuth failed").into_response();
        }
    };

    // Get user email from GitHub
    let email = match get_github_user_email(&state.http_client, &token_response.access_token).await {
        Ok(Some(email)) => email,
        Ok(None) => {
            tracing::warn!("GitHub user has no verified primary email");
            return (StatusCode::BAD_REQUEST, "GitHub account must have a verified primary email").into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get GitHub user email: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get user info").into_response();
        }
    };

    // Create or get user
    let user = match queries::upsert_user(&state.db, &email).await {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Failed to upsert user: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    // Generate API token
    let api_token = generate_api_token();
    if let Err(e) = queries::create_api_key(&state.db, user.id, &api_token, Some("CLI")).await {
        tracing::error!("Failed to create API key: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create API key").into_response();
    }

    // Check if there's a validated redirect_uri in state
    if let Some(redirect_uri) = oauth_state.redirect_uri {
        let redirect_with_token = format!("{}?token={}", redirect_uri, api_token);
        return Redirect::temporary(&redirect_with_token).into_response();
    }

    // Return token as JSON if no redirect
    Json(serde_json::json!({
        "token": api_token,
        "user": {
            "id": user.id,
            "email": user.email
        }
    }))
    .into_response()
}

/// CLI auth endpoint - redirects to GitHub with CLI redirect URI
#[derive(Debug, Deserialize)]
pub struct CliAuthQuery {
    redirect_uri: String,
}

async fn cli_auth(
    State(state): State<AppState>,
    Query(query): Query<CliAuthQuery>,
) -> Response {
    let client_id = &state.config.github_client_id;

    if client_id.is_empty() {
        return (StatusCode::SERVICE_UNAVAILABLE, "GitHub OAuth not configured").into_response();
    }

    // Validate redirect_uri (must be localhost for CLI)
    if !query.redirect_uri.starts_with("http://localhost:") && !query.redirect_uri.starts_with("http://127.0.0.1:") {
        tracing::warn!("CLI auth rejected invalid redirect_uri: {}", query.redirect_uri);
        return (StatusCode::BAD_REQUEST, "Invalid redirect_uri - must be localhost").into_response();
    }

    // Generate cryptographically secure nonce for CSRF protection
    let nonce = Uuid::new_v4().to_string();

    let oauth_state = OAuthState {
        nonce: nonce.clone(),
        redirect_uri: Some(query.redirect_uri),
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };

    // Store state in Redis with TTL
    let state_key = format!("oauth_state:{}", nonce);
    let state_json = match serde_json::to_string(&oauth_state) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("Failed to serialize OAuth state: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    if let Err(e) = state.route_manager.store_oauth_state(&state_key, &state_json).await {
        tracing::error!("Failed to store OAuth state in Redis: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
    }

    let redirect_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&scope=user:email&state={}",
        client_id, nonce
    );

    Redirect::temporary(&redirect_url).into_response()
}

/// Get current user info
#[derive(Debug, Deserialize)]
pub struct AuthHeader {
    authorization: Option<String>,
}

async fn get_user(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Response {
    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, "Missing authorization header").into_response(),
    };

    let user = match queries::find_user_by_token(&state.db, token).await {
        Ok(Some(user)) => user,
        Ok(None) => return (StatusCode::UNAUTHORIZED, "Invalid token").into_response(),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    // Check if plan has expired
    let effective_plan = if let Some(expires_at) = user.plan_expires_at {
        if expires_at < chrono::Utc::now() {
            "free" // Plan has expired
        } else {
            &user.plan
        }
    } else {
        &user.plan
    };

    Json(serde_json::json!({
        "id": user.id,
        "email": user.email,
        "created_at": user.created_at,
        "plan": effective_plan,
        "plan_expires_at": user.plan_expires_at
    }))
    .into_response()
}

/// Get usage stats
async fn get_usage(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Response {
    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, "Missing authorization header").into_response(),
    };

    let user = match queries::find_user_by_token(&state.db, token).await {
        Ok(Some(user)) => user,
        Ok(None) => return (StatusCode::UNAUTHORIZED, "Invalid token").into_response(),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    let usage = state
        .route_manager
        .get_usage(&user.id.to_string())
        .await
        .unwrap_or(0);

    // Check if plan has expired
    let effective_plan = if let Some(expires_at) = user.plan_expires_at {
        if expires_at < Utc::now() {
            "free"
        } else {
            &user.plan
        }
    } else {
        &user.plan
    };

    // Get bandwidth limit based on effective plan
    let bandwidth_limit = match effective_plan {
        "pro" => constants::BANDWIDTH_PRO,
        "hobby" => constants::BANDWIDTH_HOBBY,
        _ => constants::BANDWIDTH_FREE,
    };

    Json(serde_json::json!({
        "plan": effective_plan,
        "bandwidth_bytes": usage,
        "bandwidth_limit": bandwidth_limit,
        "plan_expires_at": user.plan_expires_at
    }))
    .into_response()
}

/// Get auth config (public endpoint for CLI)
async fn auth_config(State(state): State<AppState>) -> Response {
    Json(serde_json::json!({
        "github_client_id": state.config.github_client_id
    }))
    .into_response()
}

/// Exchange GitHub access token for Dvaar API token
#[derive(Debug, Deserialize)]
struct ExchangeTokenRequest {
    github_token: String,
}

async fn exchange_token(
    State(state): State<AppState>,
    Json(payload): Json<ExchangeTokenRequest>,
) -> Response {
    // Get user email from GitHub using the provided token
    let email = match get_github_user_email(&state.http_client, &payload.github_token).await {
        Ok(Some(email)) => email,
        Ok(None) => {
            tracing::warn!("GitHub user has no verified primary email (device flow)");
            return (StatusCode::BAD_REQUEST, "GitHub account must have a verified primary email").into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get GitHub user email: {}", e);
            return (StatusCode::UNAUTHORIZED, "Invalid GitHub token").into_response();
        }
    };

    // Create or get user
    let user = match queries::upsert_user(&state.db, &email).await {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Failed to upsert user: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    // Generate API token
    let api_token = generate_api_token();
    if let Err(e) = queries::create_api_key(&state.db, user.id, &api_token, Some("CLI")).await {
        tracing::error!("Failed to create API key: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create API key").into_response();
    }

    tracing::info!("User logged in via Device Flow: {}", email);

    Json(serde_json::json!({
        "token": api_token,
        "user": {
            "id": user.id.to_string(),
            "email": user.email
        }
    }))
    .into_response()
}

// Helper functions

fn extract_bearer_token(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
}

fn generate_api_token() -> String {
    format!("dvaar_{}", Uuid::new_v4().to_string().replace("-", ""))
}

#[derive(Debug, Deserialize)]
struct GithubTokenResponse {
    access_token: String,
    token_type: String,
}

async fn exchange_github_code(
    state: &AppState,
    code: &str,
) -> Result<GithubTokenResponse, reqwest::Error> {
    // Use shared HTTP client for connection pooling
    let response = state.http_client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", state.config.github_client_id.as_str()),
            ("client_secret", state.config.github_client_secret.as_str()),
            ("code", code),
        ])
        .send()
        .await?
        .json::<GithubTokenResponse>()
        .await?;

    Ok(response)
}

#[derive(Debug, Deserialize)]
struct GithubEmail {
    email: String,
    primary: bool,
    verified: bool,
}

async fn get_github_user_email(client: &reqwest::Client, access_token: &str) -> Result<Option<String>, reqwest::Error> {
    // Use shared HTTP client for connection pooling
    let emails: Vec<GithubEmail> = client
        .get("https://api.github.com/user/emails")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "Dvaar")
        .send()
        .await?
        .json()
        .await?;

    // Find primary verified email - return None if not found (don't fallback to unknown email)
    let email = emails
        .into_iter()
        .find(|e| e.primary && e.verified)
        .map(|e| e.email);

    Ok(email)
}

/// Get best available edge nodes for client-side routing
/// Returns top 3 nodes sorted by: 1) region match, 2) lowest load
async fn get_nodes(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Response {
    // Get client IP from Cloudflare headers
    let client_ip = headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .or_else(|| headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()).and_then(|s| s.split(',').next()))
        .unwrap_or("unknown")
        .to_string();

    // Get client region from Cloudflare header (if available)
    let client_region = headers
        .get("cf-ipcountry")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    match state.route_manager.get_all_nodes().await {
        Ok(mut nodes) => {
            // Sort nodes: prioritize same region, then by load (tunnel_count)
            nodes.sort_by(|a, b| {
                // Same region gets priority
                let a_region_match = client_region.as_ref().map(|cr| a.region.as_ref() == Some(cr)).unwrap_or(false);
                let b_region_match = client_region.as_ref().map(|cr| b.region.as_ref() == Some(cr)).unwrap_or(false);

                match (a_region_match, b_region_match) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => {
                        // Same region status - sort by load (lower is better)
                        let a_load = a.tunnel_count as f32 / a.max_tunnels.max(1) as f32;
                        let b_load = b.tunnel_count as f32 / b.max_tunnels.max(1) as f32;
                        a_load.partial_cmp(&b_load).unwrap_or(std::cmp::Ordering::Equal)
                    }
                }
            });

            // Return only top 3 available nodes
            let public_nodes: Vec<serde_json::Value> = nodes
                .into_iter()
                .filter(|n| n.tunnel_count < n.max_tunnels)
                .take(3)
                .map(|n| {
                    serde_json::json!({
                        "id": n.node_id,
                        "host": format!("{}:{}", n.ip, n.port),
                        "region": n.region,
                        "tunnels": n.tunnel_count,
                        "capacity": n.max_tunnels
                    })
                })
                .collect();

            Json(serde_json::json!({
                "nodes": public_nodes,
                "client_ip": client_ip,
                "client_region": client_region
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get nodes: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get nodes").into_response()
        }
    }
}
