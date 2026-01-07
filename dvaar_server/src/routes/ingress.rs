//! Public ingress handler - handles incoming HTTP requests to tunneled services

use crate::db::queries;
use crate::routes::{AppState, StreamChunk, TunnelCommand, TunnelRequest};
use axum::{
    body::Body,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::ConnectInfo,
    extract::State,
    http::{Request, Response, StatusCode},
    response::IntoResponse,
};
use axum_extra::extract::Host;
use dvaar_common::{constants, HttpRequestPacket, new_stream_id};
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use std::net::SocketAddr;
use tokio::sync::mpsc;
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite, MaybeTlsStream, WebSocketStream};

/// Rate limit error response
#[allow(dead_code)]
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
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request<Body>,
) -> Response<Body> {
    // Extract subdomain from host header (tunnel domain: *.dvaar.app)
    let subdomain = match extract_subdomain(&host, &state.config.tunnel_domain) {
        Some(s) => s,
        None => {
            // Check X-Subdomain header ONLY for loopback connections
            // This prevents host spoofing attacks in production
            let is_local_request = addr.ip().is_loopback();
            let allow_header_override = is_local_request && state.config.allow_subdomain_header;
            if allow_header_override {
                if let Some(header_subdomain) = request
                    .headers()
                    .get(constants::SUBDOMAIN_HEADER)
                    .and_then(|v| v.to_str().ok())
                {
                    tracing::debug!("Using X-Subdomain header for local request: {}", header_subdomain);
                    header_subdomain.to_string()
                } else {
                    return (StatusCode::NOT_FOUND, "Subdomain not found").into_response();
                }
            } else {
                // Check if this is a custom domain (CNAME) - only for non-local requests
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

/// Forward request to a local tunnel with streaming support
async fn forward_to_local_tunnel(
    handle: &crate::routes::TunnelHandle,
    request: Request<Body>,
) -> Response<Body> {
    let stream_id = new_stream_id();
    let (mut parts, body) = request.into_parts();

    let ws_upgrade = if is_websocket_upgrade_request(&parts.headers) {
        match <WebSocketUpgrade as axum::extract::FromRequestParts<()>>::from_request_parts(
            &mut parts,
            &(),
        )
        .await
        {
            Ok(upgrade) => Some(upgrade),
            Err(err) => {
                tracing::warn!("WebSocket upgrade rejected: {}", err);
                return (StatusCode::BAD_REQUEST, "Invalid WebSocket request").into_response();
            }
        }
    } else {
        None
    };

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
    };

    let (response_tx, mut response_rx) = mpsc::channel::<StreamChunk>(32);
    let tunnel_request = TunnelRequest {
        request: http_request,
        response_tx,
    };

    if handle
        .request_tx
        .send(TunnelCommand::Request(tunnel_request))
        .await
        .is_err()
    {
        return (StatusCode::BAD_GATEWAY, "Tunnel disconnected").into_response();
    }

    let request_tx = handle.request_tx.clone();
    let stream_id_for_body = stream_id.clone();
    tokio::spawn(async move {
        let mut body_stream = body.into_data_stream();
        while let Some(chunk_result) = body_stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    if request_tx
                        .send(TunnelCommand::Data {
                            stream_id: stream_id_for_body.clone(),
                            data: chunk.to_vec(),
                        })
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to read request body: {}", e);
                    break;
                }
            }
        }
        let _ = request_tx
            .send(TunnelCommand::End {
                stream_id: stream_id_for_body,
            })
            .await;
    });

    let first_chunk = match response_rx.recv().await {
        Some(chunk) => chunk,
        None => {
            return (StatusCode::BAD_GATEWAY, "No response from tunnel").into_response();
        }
    };

    let headers_packet = match first_chunk {
        StreamChunk::Headers(h) => h,
        StreamChunk::Error(e) => {
            tracing::error!("Tunnel error: {}", e);
            return (StatusCode::BAD_GATEWAY, "Tunnel error").into_response();
        }
        _ => {
            tracing::error!("Expected Headers chunk, got something else");
            return (StatusCode::BAD_GATEWAY, "Protocol error").into_response();
        }
    };

    if headers_packet.is_websocket_upgrade() {
        let Some(ws_upgrade) = ws_upgrade else {
            return (StatusCode::BAD_GATEWAY, "WebSocket upgrade failed").into_response();
        };

        let mut ws_upgrade = ws_upgrade;
        if let Some(protocol) = headers_packet
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("sec-websocket-protocol"))
            .map(|(_, v)| v.to_string())
        {
            ws_upgrade = ws_upgrade.protocols([protocol]);
        }

        let request_tx = handle.request_tx.clone();
        let stream_id_clone = stream_id.clone();
        return ws_upgrade.on_upgrade(move |socket| async move {
            bridge_websocket(socket, response_rx, request_tx, stream_id_clone).await;
        });
    }

    let status = StatusCode::from_u16(headers_packet.status).unwrap_or(StatusCode::OK);
    let mut builder = Response::builder().status(status);

    for (key, value) in &headers_packet.headers {
        builder = builder.header(key.as_str(), value.as_str());
    }

    let body_stream = async_stream::stream! {
        while let Some(chunk) = response_rx.recv().await {
            match chunk {
                StreamChunk::Data(data) => {
                    yield Ok::<_, std::io::Error>(axum::body::Bytes::from(data));
                }
                StreamChunk::End => {
                    break;
                }
                StreamChunk::Error(e) => {
                    tracing::error!("Stream error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    };

    builder
        .body(Body::from_stream(body_stream))
        .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Response build failed").into_response())
}

/// Forward request to a remote node with streaming support
async fn forward_to_remote_node(
    state: &AppState,
    subdomain: &str,
    route_info: &dvaar_common::RouteInfo,
    request: Request<Body>,
) -> Response<Body> {
    let (mut parts, body) = request.into_parts();

    if is_websocket_upgrade_request(&parts.headers) {
        let ws_upgrade = match <WebSocketUpgrade as axum::extract::FromRequestParts<()>>::from_request_parts(
            &mut parts,
            &(),
        )
        .await
        {
            Ok(upgrade) => upgrade,
            Err(err) => {
                tracing::warn!("WebSocket upgrade rejected: {}", err);
                return (StatusCode::BAD_REQUEST, "Invalid WebSocket request").into_response();
            }
        };

        let proxy_url = format!(
            "ws://{}:{}/_internal/proxy{}",
            route_info.node_ip,
            route_info.internal_port,
            parts
                .uri
                .path_and_query()
                .map(|pq| pq.to_string())
                .unwrap_or_else(|| "/".to_string())
        );

        let mut ws_request = tokio_tungstenite::tungstenite::http::Request::builder()
            .method("GET")
            .uri(&proxy_url);

        for (key, value) in &parts.headers {
            if key.as_str().eq_ignore_ascii_case("host") {
                continue;
            }
            if let Ok(v) = value.to_str() {
                ws_request = ws_request.header(key.as_str(), v);
            }
        }

        ws_request = ws_request
            .header(constants::CLUSTER_SECRET_HEADER, &state.config.cluster_secret)
            .header(
                constants::ORIGINAL_HOST_HEADER,
                state.config.full_domain(subdomain),
            );

        let ws_request = match ws_request.body(()) {
            Ok(req) => req,
            Err(e) => {
                tracing::error!("Failed to build WebSocket request: {}", e);
                return (StatusCode::BAD_GATEWAY, "WebSocket request failed").into_response();
            }
        };

        match tokio_tungstenite::connect_async(ws_request).await {
            Ok((remote_socket, response)) => {
                if response.status() != tokio_tungstenite::tungstenite::http::StatusCode::SWITCHING_PROTOCOLS {
                    return (StatusCode::BAD_GATEWAY, "WebSocket upgrade failed").into_response();
                }

                let mut ws_upgrade = ws_upgrade;
                if let Some(protocol) = response
                    .headers()
                    .get("sec-websocket-protocol")
                    .and_then(|value| value.to_str().ok())
                {
                    ws_upgrade = ws_upgrade.protocols([protocol.to_string()]);
                }

                return ws_upgrade.on_upgrade(move |socket| async move {
                    bridge_websocket_to_remote(socket, remote_socket).await;
                });
            }
            Err(e) => {
                tracing::error!("Proxy WebSocket connection failed: {}", e);
                return (StatusCode::BAD_GATEWAY, "Remote node unavailable").into_response();
            }
        }
    }

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

    let mut proxy_request = state.http_client.request(
        reqwest::Method::from_bytes(parts.method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET),
        &proxy_url,
    );

    for (key, value) in &parts.headers {
        if let Ok(v) = value.to_str() {
            proxy_request = proxy_request.header(key.as_str(), v);
        }
    }

    let body_stream = body.into_data_stream().map(|result| {
        result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    });

    proxy_request = proxy_request
        .header(constants::CLUSTER_SECRET_HEADER, &state.config.cluster_secret)
        .header(
            constants::ORIGINAL_HOST_HEADER,
            state.config.full_domain(subdomain),
        )
        .body(reqwest::Body::wrap_stream(body_stream));

    match proxy_request.send().await {
        Ok(resp) => {
            let status = resp.status();
            let headers = resp.headers().clone();

            let body_stream = resp.bytes_stream().map(|result| {
                result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
            });

            let mut builder =
                Response::builder().status(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK));

            for (key, value) in headers.iter() {
                builder = builder.header(key.as_str(), value.as_bytes());
            }

            builder
                .body(Body::from_stream(body_stream))
                .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Response build failed").into_response())
        }
        Err(e) => {
            tracing::error!("Proxy request failed: {}", e);
            (StatusCode::BAD_GATEWAY, "Remote node unavailable").into_response()
        }
    }
}

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

fn is_websocket_upgrade_request(headers: &axum::http::HeaderMap) -> bool {
    let has_upgrade_connection = headers
        .get(axum::http::header::CONNECTION)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase().contains("upgrade"))
        .unwrap_or(false);

    let has_websocket_upgrade = headers
        .get(axum::http::header::UPGRADE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);

    has_upgrade_connection && has_websocket_upgrade
}

async fn bridge_websocket(
    socket: WebSocket,
    mut response_rx: mpsc::Receiver<StreamChunk>,
    request_tx: mpsc::Sender<TunnelCommand>,
    stream_id: String,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    let to_client = tokio::spawn(async move {
        while let Some(chunk) = response_rx.recv().await {
            match chunk {
                StreamChunk::WebSocketFrame { data, is_binary } => {
                    let message = if is_binary {
                        Message::Binary(data.into())
                    } else {
                        match String::from_utf8(data) {
                            Ok(text) => Message::Text(text.into()),
                            Err(err) => {
                                tracing::warn!("Invalid UTF-8 websocket frame: {}", err);
                                continue;
                            }
                        }
                    };

                    if ws_sender.send(message).await.is_err() {
                        break;
                    }
                }
                StreamChunk::WebSocketClose { code, reason } => {
                    let close_frame = code.map(|code| axum::extract::ws::CloseFrame {
                        code,
                        reason: reason.unwrap_or_default().into(),
                    });
                    let _ = ws_sender.send(Message::Close(close_frame)).await;
                    break;
                }
                StreamChunk::Error(e) => {
                    tracing::error!("WebSocket stream error: {}", e);
                    let _ = ws_sender.send(Message::Close(None)).await;
                    break;
                }
                StreamChunk::End => {}
                _ => {}
            }
        }
    });

    let stream_id_for_tunnel = stream_id.clone();
    let to_tunnel = tokio::spawn(async move {
        let mut sent_close = false;
        while let Some(message) = ws_receiver.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    if request_tx
                        .send(TunnelCommand::WebSocketFrame {
                            stream_id: stream_id_for_tunnel.clone(),
                            data: text.as_bytes().to_vec(),
                            is_binary: false,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(Message::Binary(data)) => {
                    if request_tx
                        .send(TunnelCommand::WebSocketFrame {
                            stream_id: stream_id_for_tunnel.clone(),
                            data: data.to_vec(),
                            is_binary: true,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(Message::Close(frame)) => {
                    let (code, reason) = frame
                        .map(|frame| (Some(frame.code), Some(frame.reason.to_string())))
                        .unwrap_or((None, None));
                    let _ = request_tx
                        .send(TunnelCommand::WebSocketClose {
                            stream_id: stream_id_for_tunnel.clone(),
                            code,
                            reason,
                        })
                        .await;
                    sent_close = true;
                    break;
                }
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                Ok(_) => {}
                Err(err) => {
                    tracing::debug!("WebSocket receive error: {}", err);
                    let _ = request_tx
                        .send(TunnelCommand::WebSocketClose {
                            stream_id: stream_id_for_tunnel.clone(),
                            code: Some(1006),
                            reason: Some("WebSocket receive error".to_string()),
                        })
                        .await;
                    sent_close = true;
                    break;
                }
            }
        }

        if !sent_close {
            let _ = request_tx
                .send(TunnelCommand::WebSocketClose {
                    stream_id: stream_id_for_tunnel.clone(),
                    code: Some(1006),
                    reason: Some("Client websocket closed".to_string()),
                })
                .await;
        }
    });

    let request_tx_for_close = request_tx.clone();
    let stream_id_for_close = stream_id.clone();
    tokio::select! {
        _ = to_client => {
            let _ = request_tx_for_close
                .send(TunnelCommand::WebSocketClose {
                    stream_id: stream_id_for_close,
                    code: Some(1006),
                    reason: Some("Client websocket closed".to_string()),
                })
                .await;
            to_tunnel.abort();
        }
        _ = to_tunnel => {
            to_client.abort();
        }
    }
}

async fn bridge_websocket_to_remote(
    socket: WebSocket,
    remote_socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
) {
    let (mut client_sender, mut client_receiver) = socket.split();
    let (mut remote_sender, mut remote_receiver) = remote_socket.split();

    let client_to_remote = tokio::spawn(async move {
        while let Some(message) = client_receiver.next().await {
            let message = match message {
                Ok(message) => message,
                Err(err) => {
                    tracing::debug!("Client websocket error: {}", err);
                    break;
                }
            };

            if let Some(outgoing) = axum_to_tungstenite_message(message) {
                if remote_sender.send(outgoing).await.is_err() {
                    break;
                }
            }
        }
    });

    let remote_to_client = tokio::spawn(async move {
        while let Some(message) = remote_receiver.next().await {
            let message = match message {
                Ok(message) => message,
                Err(err) => {
                    tracing::debug!("Remote websocket error: {}", err);
                    break;
                }
            };

            if let Some(outgoing) = tungstenite_to_axum_message(message) {
                if client_sender.send(outgoing).await.is_err() {
                    break;
                }
            }
        }
    });

    tokio::select! {
        _ = client_to_remote => {
            remote_to_client.abort();
        }
        _ = remote_to_client => {
            client_to_remote.abort();
        }
    }
}

fn axum_to_tungstenite_message(message: Message) -> Option<tungstenite::Message> {
    match message {
        Message::Text(text) => Some(tungstenite::Message::Text(text.to_string())),
        Message::Binary(data) => Some(tungstenite::Message::Binary(data.to_vec())),
        Message::Ping(data) => Some(tungstenite::Message::Ping(data.to_vec())),
        Message::Pong(data) => Some(tungstenite::Message::Pong(data.to_vec())),
        Message::Close(frame) => Some(tungstenite::Message::Close(frame.map(|frame| {
            tungstenite::protocol::CloseFrame {
                code: tungstenite::protocol::frame::coding::CloseCode::from(frame.code),
                reason: frame.reason.to_string().into(),
            }
        }))),
    }
}

fn tungstenite_to_axum_message(message: tungstenite::Message) -> Option<Message> {
    match message {
        tungstenite::Message::Text(text) => Some(Message::Text(text.into())),
        tungstenite::Message::Binary(data) => Some(Message::Binary(data.into())),
        tungstenite::Message::Ping(data) => Some(Message::Ping(data.into())),
        tungstenite::Message::Pong(data) => Some(Message::Pong(data.into())),
        tungstenite::Message::Close(frame) => Some(Message::Close(frame.map(|frame| {
            axum::extract::ws::CloseFrame {
                code: u16::from(frame.code),
                reason: frame.reason.to_string().into(),
            }
        }))),
        tungstenite::Message::Frame(_) => None,
    }
}
