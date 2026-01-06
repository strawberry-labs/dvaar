//! Public ingress handler - handles incoming HTTP requests to tunneled services

use crate::db::queries;
use crate::routes::{AppState, TunnelRequest, TunnelResponse};
use axum::{
    body::Body,
    extract::State,
    http::{Request, Response, StatusCode},
    response::IntoResponse,
};
use axum_extra::extract::Host;
use dvaar_common::{constants, HttpRequestPacket, new_stream_id};
use std::time::Duration;
use tokio::sync::oneshot;

/// Rate limit error response
fn rate_limit_response(reset_in_secs: u64) -> Response<Body> {
    let body = format!(
        "Rate limit exceeded. Try again in {} seconds.",
        reset_in_secs
    );
    Response::builder()
        .status(StatusCode::TOO_MANY_REQUESTS)
        .header("Retry-After", reset_in_secs.to_string())
        .header("X-RateLimit-Reset", reset_in_secs.to_string())
        .body(Body::from(body))
        .unwrap()
}

/// Handle an incoming public HTTP request
pub async fn handle_ingress(
    State(state): State<AppState>,
    Host(host): Host,
    request: Request<Body>,
) -> Response<Body> {
    // Extract subdomain from host header (tunnel domain: *.dvaar.app)
    let subdomain = match extract_subdomain(&host, &state.config.tunnel_domain) {
        Some(s) => s,
        None => {
            // Check X-Subdomain header for local development
            if let Some(header_subdomain) = request
                .headers()
                .get(constants::SUBDOMAIN_HEADER)
                .and_then(|v| v.to_str().ok())
            {
                header_subdomain.to_string()
            } else {
                // Check if this is a custom domain (CNAME)
                let host_without_port = host.split(':').next().unwrap_or(&host);
                match queries::find_subdomain_by_custom_domain(&state.db, host_without_port).await {
                    Ok(Some(subdomain)) => subdomain,
                    Ok(None) => {
                        return (
                            StatusCode::NOT_FOUND,
                            "Domain not configured",
                        )
                            .into_response();
                    }
                    Err(e) => {
                        tracing::error!("Database error looking up custom domain: {}", e);
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal error",
                        )
                            .into_response();
                    }
                }
            }
        }
    };

    tracing::debug!("Ingress request for subdomain: {}", subdomain);

    // Request rate limiting disabled - bandwidth limits are the real gate
    // Keeping this code commented for future use if needed
    // match state.rate_limiter.check_requests(&subdomain, false).await { ... }

    // Check 1: Local tunnel
    if let Some(handle) = state.tunnels.get(&subdomain) {
        return forward_to_local_tunnel(&handle, request).await;
    }

    // Check 2: Remote node via Redis
    match state.route_manager.get_route(&subdomain).await {
        Ok(Some(route_info)) => {
            // Proxy to remote node
            forward_to_remote_node(&state, &subdomain, &route_info, request).await
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Tunnel not found").into_response(),
        Err(e) => {
            tracing::error!("Redis error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()
        }
    }
}

/// Forward request to a local tunnel
async fn forward_to_local_tunnel(
    handle: &crate::routes::TunnelHandle,
    request: Request<Body>,
) -> Response<Body> {
    // Convert HTTP request to packet
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

    // Build headers
    let headers: Vec<(String, String)> = parts
        .headers
        .iter()
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

    // Wait for response with timeout
    match tokio::time::timeout(Duration::from_secs(60), response_rx).await {
        Ok(Ok(TunnelResponse::Success(response))) => {
            build_http_response(response)
        }
        Ok(Ok(TunnelResponse::Error(e))) => {
            tracing::error!("Tunnel error: {}", e);
            (StatusCode::BAD_GATEWAY, "Tunnel error").into_response()
        }
        Ok(Ok(TunnelResponse::Timeout)) => {
            (StatusCode::GATEWAY_TIMEOUT, "Request timeout").into_response()
        }
        Ok(Err(_)) => {
            (StatusCode::BAD_GATEWAY, "Response channel closed").into_response()
        }
        Err(_) => {
            (StatusCode::GATEWAY_TIMEOUT, "Request timeout").into_response()
        }
    }
}

/// Forward request to a remote node
async fn forward_to_remote_node(
    state: &AppState,
    subdomain: &str,
    route_info: &dvaar_common::RouteInfo,
    request: Request<Body>,
) -> Response<Body> {
    let (parts, body) = request.into_parts();

    // Collect body
    let body_bytes = match axum::body::to_bytes(body, 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("Failed to read request body: {}", e);
            return (StatusCode::BAD_REQUEST, "Failed to read body").into_response();
        }
    };

    // Build proxy URL
    let proxy_url = format!(
        "http://{}:{}/_internal/proxy{}",
        route_info.node_ip,
        route_info.internal_port,
        parts
            .uri
            .path_and_query()
            .map(|pq| pq.to_string())
            .unwrap_or_else(|| "/".to_string())
    );

    // Forward request to remote node (using shared client with connection pooling)
    let mut proxy_request = state.http_client.request(
        reqwest::Method::from_bytes(parts.method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET),
        &proxy_url,
    );

    // Copy headers
    for (key, value) in &parts.headers {
        if let Ok(v) = value.to_str() {
            proxy_request = proxy_request.header(key.as_str(), v);
        }
    }

    // Add cluster headers
    proxy_request = proxy_request
        .header(constants::CLUSTER_SECRET_HEADER, &state.config.cluster_secret)
        .header(
            constants::ORIGINAL_HOST_HEADER,
            state.config.full_domain(subdomain),
        )
        .body(body_bytes.to_vec());

    match proxy_request.send().await {
        Ok(resp) => {
            let status = resp.status();
            let headers = resp.headers().clone();
            let body = resp.bytes().await.unwrap_or_default();

            let mut builder = Response::builder().status(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK));

            for (key, value) in headers.iter() {
                builder = builder.header(key.as_str(), value.as_bytes());
            }

            builder
                .body(Body::from(body))
                .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Response build failed").into_response())
        }
        Err(e) => {
            tracing::error!("Proxy request failed: {}", e);
            (StatusCode::BAD_GATEWAY, "Remote node unavailable").into_response()
        }
    }
}

// TODO: Re-enable with caching when needed
// async fn get_tunnel_owner_paid_status(state: &AppState, subdomain: &str) -> bool { ... }

/// Extract subdomain from host
fn extract_subdomain(host: &str, base_domain: &str) -> Option<String> {
    // Remove port if present
    let host = host.split(':').next()?;

    // Check if host ends with base domain
    if !host.ends_with(base_domain) {
        return None;
    }

    // Extract subdomain
    let subdomain = host.strip_suffix(base_domain)?.strip_suffix('.')?;

    if subdomain.is_empty() {
        None
    } else {
        Some(subdomain.to_string())
    }
}

/// Build HTTP response from tunnel response packet
fn build_http_response(packet: dvaar_common::HttpResponsePacket) -> Response<Body> {
    let status = StatusCode::from_u16(packet.status).unwrap_or(StatusCode::OK);
    let mut builder = Response::builder().status(status);

    for (key, value) in packet.headers {
        builder = builder.header(key, value);
    }

    builder
        .body(Body::from(packet.body))
        .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Response build failed").into_response())
}
