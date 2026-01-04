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
use serde::Deserialize;
use uuid::Uuid;

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

    // Store redirect_uri in state param if provided
    let state_param = query
        .redirect_uri
        .map(|uri| base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE, uri))
        .unwrap_or_default();

    let redirect_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&scope=user:email&state={}",
        client_id, state_param
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
    // Exchange code for access token
    let token_response = match exchange_github_code(&state, &query.code).await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::error!("Failed to exchange GitHub code: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "OAuth failed").into_response();
        }
    };

    // Get user email from GitHub
    let email = match get_github_user_email(&token_response.access_token).await {
        Ok(email) => email,
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

    // Check if there's a redirect_uri in state
    if let Some(state_param) = query.state {
        if !state_param.is_empty() {
            if let Ok(decoded) =
                base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE, &state_param)
            {
                if let Ok(redirect_uri) = String::from_utf8(decoded) {
                    let redirect_with_token = format!("{}?token={}", redirect_uri, api_token);
                    return Redirect::temporary(&redirect_with_token).into_response();
                }
            }
        }
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

    let state_param =
        base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE, &query.redirect_uri);

    let redirect_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&scope=user:email&state={}",
        client_id, state_param
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

    Json(serde_json::json!({
        "id": user.id,
        "email": user.email,
        "created_at": user.created_at,
        "plan": "free" // TODO: Implement plans
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

    Json(serde_json::json!({
        "plan": "free",
        "bandwidth_bytes": usage,
        "bandwidth_limit": "unlimited"
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
    let email = match get_github_user_email(&payload.github_token).await {
        Ok(email) => email,
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
    let client = reqwest::Client::new();
    let response = client
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

async fn get_github_user_email(access_token: &str) -> Result<String, reqwest::Error> {
    let client = reqwest::Client::new();
    let emails: Vec<GithubEmail> = client
        .get("https://api.github.com/user/emails")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "Dvaar")
        .send()
        .await?
        .json()
        .await?;

    // Find primary verified email
    let email = emails
        .into_iter()
        .find(|e| e.primary && e.verified)
        .map(|e| e.email)
        .unwrap_or_else(|| "unknown@dvaar.io".to_string());

    Ok(email)
}
