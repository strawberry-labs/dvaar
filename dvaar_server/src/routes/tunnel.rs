//! WebSocket tunnel handler with streaming support

use crate::abuse::{self, SubdomainCheck};
use crate::db::queries;
use crate::redis::{spawn_heartbeat, RouteManager};
use crate::routes::{AppState, StreamChunk, TunnelCommand, TunnelHandle};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
    routing::get,
    Router,
};
use chrono::{DateTime, Utc};
use dvaar_common::{constants, ClientHello, ControlPacket, RouteInfo, ServerHello};
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch, Mutex};

#[derive(Debug)]
struct StreamState {
    response_tx: mpsc::Sender<StreamChunk>,
    is_websocket: bool,
}

/// Build the tunnel router
pub fn router() -> Router<AppState> {
    Router::new().route("/_dvaar/tunnel", get(ws_handler))
}

/// WebSocket upgrade handler
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle a WebSocket connection
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Wait for Init packet
    let init_msg = match tokio::time::timeout(Duration::from_secs(10), receiver.next()).await {
        Ok(Some(Ok(Message::Binary(data)))) => data,
        Ok(Some(Ok(Message::Close(_)))) | Ok(None) => {
            tracing::debug!("Client disconnected before sending Init");
            return;
        }
        Ok(Some(Err(e))) => {
            tracing::error!("WebSocket error: {}", e);
            return;
        }
        Ok(Some(Ok(_))) => {
            tracing::warn!("Expected binary Init packet");
            return;
        }
        Err(_) => {
            tracing::warn!("Timeout waiting for Init packet");
            return;
        }
    };

    let init_packet = match ControlPacket::from_bytes(&init_msg) {
        Ok(ControlPacket::Init(hello)) => hello,
        Ok(_) => {
            tracing::warn!("Expected Init packet");
            return;
        }
        Err(e) => {
            tracing::error!("Failed to parse Init packet: {}", e);
            return;
        }
    };

    // Authenticate
    let user = match queries::find_user_by_token(&state.db, &init_packet.token).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            let error = ServerHello {
                assigned_domain: String::new(),
                error: Some("Invalid token".to_string()),
                server_version: constants::PROTOCOL_VERSION.to_string(),
            };
            let _ = send_packet(&mut sender, ControlPacket::InitAck(error)).await;
            return;
        }
        Err(e) => {
            tracing::error!("Database error during auth: {}", e);
            let error = ServerHello {
                assigned_domain: String::new(),
                error: Some("Authentication failed".to_string()),
                server_version: constants::PROTOCOL_VERSION.to_string(),
            };
            let _ = send_packet(&mut sender, ControlPacket::InitAck(error)).await;
            return;
        }
    };

    // Check rate limit for tunnel creation based on user's effective plan
    let is_paid = if let Some(expires_at) = user.plan_expires_at {
        if expires_at < chrono::Utc::now() {
            false
        } else {
            user.is_paid()
        }
    } else {
        user.is_paid()
    };

    match state.rate_limiter.check_tunnel_creation(&user.id.to_string(), is_paid).await {
        Ok(result) if !result.allowed => {
            tracing::warn!(
                "Rate limit exceeded for user {}: {}/{} tunnels",
                user.email,
                result.current,
                result.limit
            );
            let error = ServerHello {
                assigned_domain: String::new(),
                error: Some(format!(
                    "Rate limit exceeded. {} tunnels created in the last hour. Try again in {} seconds.",
                    result.current,
                    result.reset_in_secs
                )),
                server_version: constants::PROTOCOL_VERSION.to_string(),
            };
            let _ = send_packet(&mut sender, ControlPacket::InitAck(error)).await;
            return;
        }
        Err(e) => {
            tracing::error!("Rate limit check failed: {}", e);
        }
        Ok(_) => {}
    }

    // Check bandwidth limit
    let effective_plan = if let Some(expires_at) = user.plan_expires_at {
        if expires_at < chrono::Utc::now() {
            "free"
        } else {
            user.plan.as_str()
        }
    } else {
        user.plan.as_str()
    };

    let bandwidth_limit = match effective_plan {
        "pro" => constants::BANDWIDTH_PRO,
        "hobby" => constants::BANDWIDTH_HOBBY,
        _ => constants::BANDWIDTH_FREE,
    };

    match state.route_manager.get_usage(&user.id.to_string()).await {
        Ok(current_usage) if current_usage >= bandwidth_limit => {
            let limit_gb = bandwidth_limit / (1024 * 1024 * 1024);
            tracing::warn!(
                "Bandwidth limit exceeded for user {}: {} bytes / {} GB",
                user.email,
                current_usage,
                limit_gb
            );
            let error = ServerHello {
                assigned_domain: String::new(),
                error: Some(format!(
                    "Monthly bandwidth limit exceeded ({} GB). Upgrade your plan at https://dvaar.io/billing",
                    limit_gb
                )),
                server_version: constants::PROTOCOL_VERSION.to_string(),
            };
            let _ = send_packet(&mut sender, ControlPacket::InitAck(error)).await;
            return;
        }
        Err(e) => {
            tracing::error!("Bandwidth check failed: {}", e);
        }
        Ok(_) => {}
    }

    // Determine concurrent tunnel limit for this plan
    let concurrent_limit = match effective_plan {
        "pro" => constants::CONCURRENT_TUNNELS_PRO,
        "hobby" => constants::CONCURRENT_TUNNELS_HOBBY,
        _ => constants::CONCURRENT_TUNNELS_FREE,
    };

    // Generate or validate subdomain
    let can_request_subdomain = matches!(effective_plan, "hobby" | "pro");
    let subdomain = match assign_subdomain(&state, &init_packet, &user.id.to_string(), can_request_subdomain).await {
        Ok(s) => s,
        Err(e) => {
            let error = ServerHello {
                assigned_domain: String::new(),
                error: Some(e),
                server_version: constants::PROTOCOL_VERSION.to_string(),
            };
            let _ = send_packet(&mut sender, ControlPacket::InitAck(error)).await;
            return;
        }
    };

    let full_domain = state.config.full_domain(&subdomain);
    let full_url = state.config.full_url(&subdomain);

    // Register route in Redis
    let route_info = RouteInfo::new(
        state.config.node_ip.clone(),
        state.config.internal_port,
        user.id.to_string(),
    );

    if let Err(e) = state.route_manager.register_route(&subdomain, &route_info).await {
        tracing::error!("Failed to register route: {}", e);
        let error = ServerHello {
            assigned_domain: String::new(),
            error: Some("Failed to register route".to_string()),
            server_version: constants::PROTOCOL_VERSION.to_string(),
        };
        let _ = send_packet(&mut sender, ControlPacket::InitAck(error)).await;
        return;
    }

    // Register tunnel in sorted set (tracks individual tunnels with timestamps)
    // Stale tunnels auto-expire after 1 min if heartbeat stops
    let user_id_for_cleanup = user.id.to_string();
    match state.route_manager.register_user_tunnel(&user_id_for_cleanup, &subdomain, concurrent_limit).await {
        Ok((current_tunnels, false)) => {
            // Over limit
            tracing::warn!(
                "Concurrent tunnel limit exceeded for user {}: {}/{} tunnels",
                user.email,
                current_tunnels,
                concurrent_limit
            );
            // Clean up the route we just registered
            let _ = state.route_manager.remove_route(&subdomain).await;

            let upgrade_msg = match effective_plan {
                "free" => "Upgrade to Hobby ($5/mo) for 10 concurrent tunnels: dvaar upgrade",
                "hobby" => "Upgrade to Pro ($15/mo) for 50 concurrent tunnels: dvaar upgrade",
                _ => "You've reached the maximum concurrent tunnels for your plan",
            };
            let error = ServerHello {
                assigned_domain: String::new(),
                error: Some(format!(
                    "Maximum {} concurrent tunnels reached. {}",
                    concurrent_limit, upgrade_msg
                )),
                server_version: constants::PROTOCOL_VERSION.to_string(),
            };
            let _ = send_packet(&mut sender, ControlPacket::InitAck(error)).await;
            return;
        }
        Err(e) => {
            tracing::error!("Concurrent tunnel check failed: {}", e);
            // On Redis error, allow the tunnel (fail open) but log it
        }
        Ok((_, true)) => {
            // Successfully registered, we're under the limit
        }
    }

    // Send success response
    let ack = ServerHello {
        assigned_domain: full_domain.clone(),
        error: None,
        server_version: constants::PROTOCOL_VERSION.to_string(),
    };

    if send_packet(&mut sender, ControlPacket::InitAck(ack)).await.is_err() {
        // InitAck failed - clean up route AND unregister tunnel
        let _ = state.route_manager.remove_route(&subdomain).await;
        let _ = state.route_manager.unregister_user_tunnel(&user_id_for_cleanup, &subdomain).await;
        return;
    }

    tracing::info!(
        "Tunnel established: {} -> {} (user: {})",
        full_url,
        init_packet.tunnel_type.as_str(),
        user.email
    );

    // Create channels for request/response handling
    let (request_tx, mut request_rx) = mpsc::channel::<TunnelCommand>(32);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Register local tunnel handle
    state.tunnels.insert(
        subdomain.clone(),
        TunnelHandle {
            request_tx,
            user_id: user.id.to_string(),
        },
    );

    // Start heartbeat task (also refreshes user tunnel count TTL)
    let heartbeat_handle = spawn_heartbeat(
        RouteManager::new(state.redis.clone()),
        subdomain.clone(),
        user_id_for_cleanup.clone(),
        shutdown_rx,
    );

    // Active streams: stream_id -> response channel
    let active_streams: Arc<Mutex<HashMap<String, StreamState>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // WebSocket sender shared between tasks
    let sender = Arc::new(Mutex::new(sender));
    let sender_clone = sender.clone();

    // Task to send requests to client
    let active_streams_clone = active_streams.clone();
    let send_task = tokio::spawn(async move {
        while let Some(command) = request_rx.recv().await {
            match command {
                TunnelCommand::Request(tunnel_req) => {
                    let stream_id = tunnel_req.request.stream_id.clone();

                    {
                        let mut streams = active_streams_clone.lock().await;
                        streams.insert(
                            stream_id.clone(),
                            StreamState {
                                response_tx: tunnel_req.response_tx,
                                is_websocket: false,
                            },
                        );
                    }

                    let packet = ControlPacket::HttpRequest(tunnel_req.request);
                    let send_result = {
                        let mut sender = sender_clone.lock().await;
                        send_packet(&mut *sender, packet).await
                    };

                    if send_result.is_err() {
                        let tx = {
                            let mut streams = active_streams_clone.lock().await;
                            streams.remove(&stream_id).map(|state| state.response_tx)
                        };
                        if let Some(tx) = tx {
                            let _ = tx
                                .send(StreamChunk::Error("Send failed".to_string()))
                                .await;
                        }
                        break;
                    }
                }
                TunnelCommand::Data { stream_id, data } => {
                    let packet = ControlPacket::Data {
                        stream_id: stream_id.clone(),
                        data,
                    };
                    let send_result = {
                        let mut sender = sender_clone.lock().await;
                        send_packet(&mut *sender, packet).await
                    };
                    if send_result.is_err() {
                        let tx = {
                            let mut streams = active_streams_clone.lock().await;
                            streams.remove(&stream_id).map(|state| state.response_tx)
                        };
                        if let Some(tx) = tx {
                            let _ = tx
                                .send(StreamChunk::Error("Send failed".to_string()))
                                .await;
                        }
                        break;
                    }
                }
                TunnelCommand::End { stream_id } => {
                    let packet = ControlPacket::End {
                        stream_id: stream_id.clone(),
                    };
                    let send_result = {
                        let mut sender = sender_clone.lock().await;
                        send_packet(&mut *sender, packet).await
                    };
                    if send_result.is_err() {
                        let tx = {
                            let mut streams = active_streams_clone.lock().await;
                            streams.remove(&stream_id).map(|state| state.response_tx)
                        };
                        if let Some(tx) = tx {
                            let _ = tx
                                .send(StreamChunk::Error("Send failed".to_string()))
                                .await;
                        }
                        break;
                    }
                }
                TunnelCommand::WebSocketFrame {
                    stream_id,
                    data,
                    is_binary,
                } => {
                    let packet = ControlPacket::WebSocketFrame {
                        stream_id: stream_id.clone(),
                        data,
                        is_binary,
                    };
                    let send_result = {
                        let mut sender = sender_clone.lock().await;
                        send_packet(&mut *sender, packet).await
                    };
                    if send_result.is_err() {
                        let tx = {
                            let mut streams = active_streams_clone.lock().await;
                            streams.remove(&stream_id).map(|state| state.response_tx)
                        };
                        if let Some(tx) = tx {
                            let _ = tx
                                .send(StreamChunk::Error("Send failed".to_string()))
                                .await;
                        }
                        break;
                    }
                }
                TunnelCommand::WebSocketClose {
                    stream_id,
                    code,
                    reason,
                } => {
                    let packet = ControlPacket::WebSocketClose {
                        stream_id: stream_id.clone(),
                        code,
                        reason,
                    };
                    let send_result = {
                        let mut sender = sender_clone.lock().await;
                        send_packet(&mut *sender, packet).await
                    };
                    if send_result.is_err() {
                        let tx = {
                            let mut streams = active_streams_clone.lock().await;
                            streams.remove(&stream_id).map(|state| state.response_tx)
                        };
                        if let Some(tx) = tx {
                            let _ = tx
                                .send(StreamChunk::Error("Send failed".to_string()))
                                .await;
                        }
                        break;
                    }
                }
            }
        }
    });

    // Task to receive responses from client
    let active_streams_clone = active_streams.clone();
    let route_manager_clone = state.route_manager.clone();
    let usage_is_paid = matches!(effective_plan, "hobby" | "pro");
    let usage_plan_expires_at = user.plan_expires_at;

    let recv_task = tokio::spawn(async move {
        let mut bandwidth_buffer = 0u64;
        let user_id = user.id.to_string();

        while let Some(msg) = receiver.next().await {
            let data = match msg {
                Ok(Message::Binary(data)) => data,
                Ok(Message::Ping(data)) => {
                    let mut sender = sender.lock().await;
                    let _ = sender.send(Message::Pong(data)).await;
                    continue;
                }
                Ok(Message::Pong(_)) => continue,
                Ok(Message::Close(_)) | Err(_) => break,
                Ok(Message::Text(_)) => continue,
            };

            // Track bandwidth
            bandwidth_buffer += data.len() as u64;
            if bandwidth_buffer >= 1_000_000 {
                let usage_ttl_secs = usage_ttl_secs(usage_is_paid, usage_plan_expires_at);
                let _ = route_manager_clone
                    .increment_usage(&user_id, bandwidth_buffer, usage_ttl_secs)
                    .await;
                bandwidth_buffer = 0;
            }

            let packet = match ControlPacket::from_bytes(&data) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("Failed to parse packet: {}", e);
                    continue;
                }
            };

            match packet {
                ControlPacket::HttpResponse(response) => {
                    let (tx, is_websocket) = {
                        let mut streams = active_streams_clone.lock().await;
                        if let Some(state) = streams.get_mut(&response.stream_id) {
                            if response.is_websocket_upgrade() {
                                state.is_websocket = true;
                            }
                            (Some(state.response_tx.clone()), state.is_websocket)
                        } else {
                            (None, false)
                        }
                    };

                    if let Some(tx) = tx {
                        let _ = tx.send(StreamChunk::Headers(response)).await;
                    }

                    if is_websocket {
                        continue;
                    }
                }

                ControlPacket::Data { stream_id, data } => {
                    let tx = {
                        let streams = active_streams_clone.lock().await;
                        streams.get(&stream_id).map(|state| state.response_tx.clone())
                    };
                    if let Some(tx) = tx {
                        let _ = tx.send(StreamChunk::Data(data)).await;
                    }
                }

                ControlPacket::End { stream_id } => {
                    let tx = {
                        let mut streams = active_streams_clone.lock().await;
                        match streams.get(&stream_id) {
                            Some(state) if state.is_websocket => None,
                            Some(_) => streams.remove(&stream_id).map(|state| state.response_tx),
                            None => None,
                        }
                    };
                    if let Some(tx) = tx {
                        let _ = tx.send(StreamChunk::End).await;
                    }
                }

                ControlPacket::WebSocketFrame { stream_id, data, is_binary } => {
                    let tx = {
                        let streams = active_streams_clone.lock().await;
                        streams.get(&stream_id).map(|state| state.response_tx.clone())
                    };
                    if let Some(tx) = tx {
                        let _ = tx
                            .send(StreamChunk::WebSocketFrame { data, is_binary })
                            .await;
                    }
                }

                ControlPacket::WebSocketClose { stream_id, code, reason } => {
                    let tx = {
                        let mut streams = active_streams_clone.lock().await;
                        streams.remove(&stream_id).map(|state| state.response_tx)
                    };
                    if let Some(tx) = tx {
                        let _ = tx.send(StreamChunk::WebSocketClose { code, reason }).await;
                    }
                }

                ControlPacket::StreamError { stream_id, error } => {
                    let tx = {
                        let mut streams = active_streams_clone.lock().await;
                        streams.remove(&stream_id).map(|state| state.response_tx)
                    };
                    if let Some(tx) = tx {
                        let _ = tx.send(StreamChunk::Error(error)).await;
                    }
                }

                ControlPacket::Ping => {
                    let mut sender = sender.lock().await;
                    let _ = send_packet(&mut *sender, ControlPacket::Pong).await;
                }

                ControlPacket::Pong => {}

                _ => {
                    tracing::debug!("Unexpected packet type from client");
                }
            }
        }

        // Flush remaining bandwidth
        if bandwidth_buffer > 0 {
            let usage_ttl_secs = usage_ttl_secs(usage_is_paid, usage_plan_expires_at);
            let _ = route_manager_clone
                .increment_usage(&user_id, bandwidth_buffer, usage_ttl_secs)
                .await;
        }

        // Close all active streams
        let mut streams = active_streams_clone.lock().await;
        for (_, state) in streams.drain() {
            let _ = state.response_tx.send(StreamChunk::Error("Tunnel closed".to_string())).await;
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }

    // Cleanup
    let _ = shutdown_tx.send(true);
    heartbeat_handle.abort();
    state.tunnels.remove(&subdomain);
    let _ = state.route_manager.remove_route(&subdomain).await;
    let _ = state.route_manager.unregister_user_tunnel(&user_id_for_cleanup, &subdomain).await;

    tracing::info!("Tunnel closed: {}", full_domain);
}

/// Send a control packet
async fn send_packet(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    packet: ControlPacket,
) -> Result<(), axum::Error> {
    let data = packet.to_bytes().map_err(|e| {
        tracing::error!("Failed to serialize packet: {}", e);
        axum::Error::new(e)
    })?;
    sender.send(Message::Binary(data.into())).await?;
    Ok(())
}

/// Assign a subdomain (generate random if not requested)
async fn assign_subdomain(
    state: &AppState,
    init: &ClientHello,
    user_id: &str,
    can_request_subdomain: bool,
) -> Result<String, String> {
    if let Some(requested) = &init.requested_subdomain {
        if !can_request_subdomain {
            return Err("Custom subdomains require a paid plan".to_string());
        }

        match abuse::check_subdomain(requested) {
            SubdomainCheck::Blocked(reason) => {
                tracing::warn!(
                    "Blocked subdomain request '{}' from user {}: {:?}",
                    requested,
                    user_id,
                    reason
                );
                return Err(reason.message());
            }
            SubdomainCheck::Allowed => {}
        }

        if let Ok(Some(route)) = state.route_manager.get_route(requested).await {
            if route.user_id != user_id {
                return Err("Subdomain is in use by another user".to_string());
            }
        }

        if let Ok(Some(domain)) = queries::check_subdomain_owner(&state.db, requested).await {
            if domain.user_id.to_string() != user_id {
                return Err("Subdomain is reserved by another user".to_string());
            }
        }

        Ok(requested.clone())
    } else {
        let subdomain = generate_random_subdomain();
        Ok(subdomain)
    }
}

/// Generate a random subdomain
fn generate_random_subdomain() -> String {
    let adjectives = [
        "quick", "lazy", "happy", "sad", "bright", "dark", "cool", "warm", "fast", "slow",
        "red", "blue", "green", "bold", "calm", "wild", "soft", "loud", "tiny", "huge",
    ];
    let nouns = [
        "fox", "dog", "cat", "bird", "fish", "bear", "wolf", "deer", "hawk", "owl",
        "tree", "lake", "hill", "rock", "wave", "star", "moon", "sun", "cloud", "rain",
    ];

    let mut rng = rand::thread_rng();
    let adj = adjectives[rng.gen_range(0..adjectives.len())];
    let noun = nouns[rng.gen_range(0..nouns.len())];
    let num: u16 = rng.gen_range(100..999);

    format!("{}-{}-{}", adj, noun, num)
}

fn usage_ttl_secs(is_paid: bool, plan_expires_at: Option<DateTime<Utc>>) -> i64 {
    const FREE_USAGE_TTL_SECS: i64 = 30 * 24 * 60 * 60;

    if is_paid {
        if let Some(expires_at) = plan_expires_at {
            let ttl = (expires_at - Utc::now()).num_seconds();
            if ttl > 0 {
                return ttl;
            }
        }
    }

    FREE_USAGE_TTL_SECS
}
