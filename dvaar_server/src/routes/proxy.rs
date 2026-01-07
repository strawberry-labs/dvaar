//! Internal node-to-node proxy handler

use crate::routes::{AppState, StreamChunk, TunnelCommand, TunnelRequest};
use axum::{
    body::Body,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    http::{Request, Response, StatusCode},
    response::IntoResponse,
    routing::any,
    Router,
};
use dvaar_common::{constants, HttpRequestPacket, new_stream_id};
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use tokio::sync::mpsc;

/// Build the internal proxy router
pub fn router() -> Router<AppState> {
    Router::new().route("/_internal/proxy", any(handle_internal_proxy))
    .route("/_internal/proxy/{*path}", any(handle_internal_proxy))
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
    // Note: Ingress sends X-Original-Host as subdomain.tunnel_domain (e.g., foo.dvaar.app)
    let original_host = request
        .headers()
        .get(constants::ORIGINAL_HOST_HEADER)
        .and_then(|v| v.to_str().ok());

    let subdomain = match original_host {
        Some(host) => extract_subdomain_from_host(host, &state.config.tunnel_domain),
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
        .filter(|(k, _)| {
            !k.as_str().eq_ignore_ascii_case(constants::CLUSTER_SECRET_HEADER)
                && !k
                    .as_str()
                    .eq_ignore_ascii_case(constants::ORIGINAL_HOST_HEADER)
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
