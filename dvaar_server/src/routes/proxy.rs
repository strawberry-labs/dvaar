//! Internal node-to-node proxy handler

use crate::routes::{AppState, TunnelRequest, TunnelResponse};
use axum::{
    body::Body,
    extract::State,
    http::{Request, Response, StatusCode},
    response::IntoResponse,
    routing::any,
    Router,
};
use dvaar_common::{constants, HttpRequestPacket, new_stream_id};
use std::time::Duration;
use tokio::sync::oneshot;

/// Build the internal proxy router
pub fn router() -> Router<AppState> {
    Router::new().route("/_internal/proxy", any(handle_internal_proxy))
    .route("/_internal/proxy/*path", any(handle_internal_proxy))
}

/// Handle internal proxy request from another node
async fn handle_internal_proxy(
    State(state): State<AppState>,
    request: Request<Body>,
) -> Response<Body> {
    // Validate cluster secret
    let cluster_secret = request
        .headers()
        .get(constants::CLUSTER_SECRET_HEADER)
        .and_then(|v| v.to_str().ok());

    match cluster_secret {
        Some(secret) if secret == state.config.cluster_secret => {}
        _ => {
            return (StatusCode::FORBIDDEN, "Invalid cluster secret").into_response();
        }
    }

    // Get original host to determine subdomain
    let original_host = request
        .headers()
        .get(constants::ORIGINAL_HOST_HEADER)
        .and_then(|v| v.to_str().ok());

    let subdomain = match original_host {
        Some(host) => extract_subdomain_from_host(host, &state.config.base_domain),
        None => {
            return (StatusCode::BAD_REQUEST, "Missing X-Original-Host header").into_response();
        }
    };

    let subdomain = match subdomain {
        Some(s) => s,
        None => {
            return (StatusCode::BAD_REQUEST, "Invalid X-Original-Host").into_response();
        }
    };

    // Find local tunnel
    let handle = match state.tunnels.get(&subdomain) {
        Some(h) => h,
        None => {
            return (StatusCode::NOT_FOUND, "Tunnel not found on this node").into_response();
        }
    };

    // Forward to tunnel
    let stream_id = new_stream_id();
    let (parts, body) = request.into_parts();

    // Collect body
    let body_bytes = match axum::body::to_bytes(body, 10 * 1024 * 1024).await {
        Ok(b) => b.to_vec(),
        Err(e) => {
            tracing::error!("Failed to read request body: {}", e);
            return (StatusCode::BAD_REQUEST, "Failed to read body").into_response();
        }
    };

    // Build headers (filter out internal headers)
    let headers: Vec<(String, String)> = parts
        .headers
        .iter()
        .filter(|(k, _)| {
            let key = k.as_str();
            key != constants::CLUSTER_SECRET_HEADER.to_lowercase()
                && key != constants::ORIGINAL_HOST_HEADER.to_lowercase()
        })
        .filter_map(|(k, v)| v.to_str().ok().map(|s| (k.to_string(), s.to_string())))
        .collect();

    let http_request = HttpRequestPacket {
        stream_id: stream_id.clone(),
        method: parts.method.to_string(),
        uri: parts
            .uri
            .path_and_query()
            .map(|pq| pq.to_string())
            .unwrap_or_else(|| "/".to_string()),
        headers,
        body: body_bytes,
    };

    // Create response channel
    let (response_tx, response_rx) = oneshot::channel();

    let tunnel_request = TunnelRequest {
        request: http_request,
        response_tx,
    };

    // Send to tunnel
    if handle.request_tx.send(tunnel_request).await.is_err() {
        return (StatusCode::BAD_GATEWAY, "Tunnel disconnected").into_response();
    }

    // Wait for response
    match tokio::time::timeout(Duration::from_secs(60), response_rx).await {
        Ok(Ok(TunnelResponse::Success(response))) => {
            let status = StatusCode::from_u16(response.status).unwrap_or(StatusCode::OK);
            let mut builder = Response::builder().status(status);

            for (key, value) in response.headers {
                builder = builder.header(key, value);
            }

            builder
                .body(Body::from(response.body))
                .unwrap_or_else(|_| {
                    (StatusCode::INTERNAL_SERVER_ERROR, "Response build failed").into_response()
                })
        }
        Ok(Ok(TunnelResponse::Error(e))) => {
            tracing::error!("Tunnel error: {}", e);
            (StatusCode::BAD_GATEWAY, "Tunnel error").into_response()
        }
        Ok(Ok(TunnelResponse::Timeout)) => {
            (StatusCode::GATEWAY_TIMEOUT, "Request timeout").into_response()
        }
        Ok(Err(_)) => (StatusCode::BAD_GATEWAY, "Response channel closed").into_response(),
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "Request timeout").into_response(),
    }
}

/// Extract subdomain from host
fn extract_subdomain_from_host(host: &str, base_domain: &str) -> Option<String> {
    let host = host.split(':').next()?;

    if !host.ends_with(base_domain) {
        return None;
    }

    let subdomain = host.strip_suffix(base_domain)?.strip_suffix('.')?;

    if subdomain.is_empty() {
        None
    } else {
        Some(subdomain.to_string())
    }
}
