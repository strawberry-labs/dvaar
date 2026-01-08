//! Inspector HTTP server with WebSocket support

use super::html::INSPECTOR_HTML;
use super::store::RequestStore;
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
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
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
    // We don't know the upstream yet, but we'll set a placeholder
    // The replay function will get this from TunnelClient context
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
        store,
        upstream_addr: Arc::new(upstream_addr),
        upstream_tls,
    };

    let app = Router::new()
        .route("/", get(serve_dashboard))
        .route("/api/requests", get(get_requests))
        .route("/api/requests/{id}", get(get_request))
        .route("/api/replay/{id}", post(replay_request))
        .route("/api/clear", post(clear_requests))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context(format!("Failed to bind inspector to {}", addr))?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    Ok(handle)
}

/// Serve the HTML dashboard
async fn serve_dashboard() -> Html<&'static str> {
    Html(INSPECTOR_HTML)
}

/// Get all captured requests
async fn get_requests(State(state): State<AppState>) -> Json<Vec<super::store::CapturedRequest>> {
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
