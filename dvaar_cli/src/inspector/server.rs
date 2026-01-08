//! Inspector HTTP server with WebSocket support

use super::html::INSPECTOR_HTML;
use super::store::{CapturedRequest, RegisteredTunnel, RequestStore, TunnelStatus};
use anyhow::{Context, Result};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::task::JoinHandle;

/// App state for the inspector server
#[derive(Clone)]
struct AppState {
    store: Arc<RequestStore>,
    upstream_addr: Arc<String>,
    upstream_tls: bool,
}

/// Start the inspector server on the given port
pub async fn start_server(port: u16, store: Arc<RequestStore>) -> Result<JoinHandle<()>> {
    start_server_with_upstream(port, store, String::new(), false).await
}

/// Start the inspector server with upstream info for replay
pub async fn start_server_with_upstream(
    port: u16,
    store: Arc<RequestStore>,
    upstream_addr: String,
    upstream_tls: bool,
) -> Result<JoinHandle<()>> {
    let state = AppState {
        store: store.clone(),
        upstream_addr: Arc::new(upstream_addr),
        upstream_tls,
    };

    let app = Router::new()
        // Dashboard
        .route("/", get(serve_dashboard))
        // Health check (for port detection)
        .route("/api/health", get(health_check))
        // Legacy endpoints
        .route("/api/requests", get(get_requests))
        .route("/api/requests/{id}", get(get_request))
        .route("/api/replay/{id}", post(replay_request))
        .route("/api/clear", post(clear_requests))
        .route("/api/metrics", get(get_metrics))
        .route("/api/info", get(get_info))
        // Multi-tunnel endpoints
        .route("/api/tunnels", get(get_tunnels))
        .route("/api/tunnels/register", post(register_tunnel))
        .route("/api/tunnels/{tunnel_id}/unregister", post(unregister_tunnel))
        .route("/api/tunnels/{tunnel_id}/heartbeat", post(heartbeat))
        .route("/api/tunnels/{tunnel_id}/request", post(submit_request))
        .route("/api/tunnels/{tunnel_id}/requests", get(get_tunnel_requests))
        .route("/api/tunnels/{tunnel_id}/metrics", get(get_tunnel_metrics))
        // WebSocket
        .route("/ws", get(ws_handler))
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context(format!("Failed to bind inspector to {}", addr))?;

    // Start cleanup task for stale tunnels
    let cleanup_store = store.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            cleanup_store.cleanup_stale_tunnels(120).await; // 2 minute threshold
        }
    });

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    Ok(handle)
}

// ============================================================================
// Health Check
// ============================================================================

/// Health check response
#[derive(Serialize)]
struct HealthResponse {
    service: String,
    version: String,
    tunnels: usize,
}

/// Health check endpoint for port detection
async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    let tunnels = state.store.get_tunnels().await;
    Json(HealthResponse {
        service: "dvaar-inspector".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        tunnels: tunnels.len(),
    })
}

// ============================================================================
// Multi-Tunnel Endpoints
// ============================================================================

/// Request to register a tunnel
#[derive(Debug, Deserialize)]
struct RegisterTunnelRequest {
    tunnel_id: String,
    subdomain: String,
    public_url: String,
    local_addr: String,
}

/// Response from tunnel registration
#[derive(Debug, Serialize)]
struct RegisterTunnelResponse {
    success: bool,
    tunnel_id: String,
}

/// Register a new tunnel
async fn register_tunnel(
    State(state): State<AppState>,
    Json(req): Json<RegisterTunnelRequest>,
) -> Json<RegisterTunnelResponse> {
    let tunnel = RegisteredTunnel {
        tunnel_id: req.tunnel_id.clone(),
        subdomain: req.subdomain,
        public_url: req.public_url,
        local_addr: req.local_addr,
        status: TunnelStatus::Active,
        registered_at: Utc::now(),
        last_seen: Utc::now(),
    };

    state.store.register_tunnel(tunnel).await;

    Json(RegisterTunnelResponse {
        success: true,
        tunnel_id: req.tunnel_id,
    })
}

/// Unregister a tunnel
async fn unregister_tunnel(
    State(state): State<AppState>,
    Path(tunnel_id): Path<String>,
) -> StatusCode {
    state.store.unregister_tunnel(&tunnel_id).await;
    StatusCode::OK
}

/// Heartbeat from a tunnel
async fn heartbeat(
    State(state): State<AppState>,
    Path(tunnel_id): Path<String>,
) -> StatusCode {
    state.store.heartbeat(&tunnel_id).await;
    StatusCode::OK
}

/// Submit a request from a remote tunnel
async fn submit_request(
    State(state): State<AppState>,
    Path(tunnel_id): Path<String>,
    Json(request): Json<CapturedRequest>,
) -> StatusCode {
    state.store.add_request_for_tunnel(&tunnel_id, request).await;
    StatusCode::OK
}

/// Get all tunnels
async fn get_tunnels(State(state): State<AppState>) -> Json<Vec<RegisteredTunnel>> {
    Json(state.store.get_tunnels().await)
}

/// Get requests for a specific tunnel
async fn get_tunnel_requests(
    State(state): State<AppState>,
    Path(tunnel_id): Path<String>,
) -> Json<Vec<CapturedRequest>> {
    Json(state.store.get_requests_for_tunnel(Some(&tunnel_id)).await)
}

/// Get metrics for a specific tunnel
async fn get_tunnel_metrics(
    State(state): State<AppState>,
    Path(tunnel_id): Path<String>,
) -> Response {
    match state.store.get_tunnel_metrics(&tunnel_id).await {
        Some(metrics) => Json(metrics).into_response(),
        None => (StatusCode::NOT_FOUND, "Tunnel not found").into_response(),
    }
}

// ============================================================================
// Legacy Endpoints
// ============================================================================

/// Serve the HTML dashboard
async fn serve_dashboard() -> Html<&'static str> {
    Html(INSPECTOR_HTML)
}

/// Get all captured requests
async fn get_requests(State(state): State<AppState>) -> Json<Vec<CapturedRequest>> {
    Json(state.store.get_requests().await)
}

/// Get a single request by ID
async fn get_request(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    match state.store.get_request(&id).await {
        Some(req) => Json(req).into_response(),
        None => (StatusCode::NOT_FOUND, "Request not found").into_response(),
    }
}

/// Get metrics snapshot
async fn get_metrics(State(state): State<AppState>) -> Json<crate::metrics::MetricsSnapshot> {
    Json(state.store.get_metrics().await)
}

/// Get tunnel info for status page
async fn get_info(State(state): State<AppState>) -> Json<super::store::TunnelInfoData> {
    Json(state.store.get_tunnel_info().await)
}

#[derive(Deserialize)]
struct ReplayBody {
    upstream_addr: Option<String>,
    upstream_tls: Option<bool>,
}

/// Replay a captured request
async fn replay_request(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<ReplayBody>>,
) -> Response {
    let request = match state.store.get_request(&id).await {
        Some(req) => req,
        None => return (StatusCode::NOT_FOUND, "Request not found").into_response(),
    };

    // Get upstream from body or state
    let upstream_addr = body
        .as_ref()
        .and_then(|b| b.upstream_addr.clone())
        .unwrap_or_else(|| (*state.upstream_addr).clone());

    let upstream_tls = body
        .as_ref()
        .and_then(|b| b.upstream_tls)
        .unwrap_or(state.upstream_tls);

    if upstream_addr.is_empty() {
        return (StatusCode::BAD_REQUEST, "Upstream address not configured").into_response();
    }

    // Build and send the request
    let scheme = if upstream_tls { "https" } else { "http" };
    let url = format!("{}://{}{}", scheme, upstream_addr, request.path);

    let client = reqwest::Client::new();
    let method = match request.method.as_str() {
        "GET" => reqwest::Method::GET,
        "POST" => reqwest::Method::POST,
        "PUT" => reqwest::Method::PUT,
        "DELETE" => reqwest::Method::DELETE,
        "PATCH" => reqwest::Method::PATCH,
        "HEAD" => reqwest::Method::HEAD,
        "OPTIONS" => reqwest::Method::OPTIONS,
        _ => reqwest::Method::GET,
    };

    let mut req_builder = client.request(method, &url);

    // Add original headers (except host)
    for (key, value) in &request.request_headers {
        if key.to_lowercase() != "host" {
            req_builder = req_builder.header(key.as_str(), value.as_str());
        }
    }

    // Add body if present
    if !request.request_body.is_empty() {
        req_builder = req_builder.body(request.request_body.clone());
    }

    match req_builder.send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            Json(serde_json::json!({
                "success": true,
                "status": status,
                "message": format!("Replayed request, got status {}", status)
            }))
            .into_response()
        }
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))
        .into_response(),
    }
}

/// Clear all captured requests
async fn clear_requests(State(state): State<AppState>) -> StatusCode {
    state.store.clear().await;
    StatusCode::OK
}

// ============================================================================
// WebSocket
// ============================================================================

/// WebSocket handler for live updates
async fn ws_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_websocket(socket, state))
}

/// Handle a WebSocket connection
async fn handle_websocket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Send initial batch of requests
    let requests = state.store.get_requests().await;
    let initial_msg = serde_json::json!({
        "type": "requests",
        "data": requests
    });
    if let Ok(json) = serde_json::to_string(&initial_msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Send initial tunnels list
    let tunnels = state.store.get_tunnels().await;
    let tunnels_msg = serde_json::json!({
        "type": "tunnels",
        "data": tunnels
    });
    if let Ok(json) = serde_json::to_string(&tunnels_msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Subscribe to new events
    let mut event_rx = state.store.subscribe();

    // Spawn task to forward events to WebSocket
    let send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            if let Ok(json) = serde_json::to_string(&event) {
                if sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Handle incoming messages (for future extensions like replay from WS)
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(data)) => {
                // Pong is handled automatically by axum
                let _ = data;
            }
            _ => {}
        }
    }

    send_task.abort();
}
