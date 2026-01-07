//! WebSocket tunnel client with streaming and WebSocket passthrough support

use anyhow::{Context, Result};
use bytes::Bytes;
use console::style;
use dvaar_common::{
    constants, ClientHello, ControlPacket, HttpRequestPacket, HttpResponsePacket, TunnelType,
};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{self, Message},
    MaybeTlsStream, WebSocketStream,
};

/// Chunk size for streaming (64KB)
const STREAM_CHUNK_SIZE: usize = 64 * 1024;

/// Tunnel client for HTTP tunneling with streaming support
pub struct TunnelClient {
    server_url: String,
    token: String,
    requested_subdomain: Option<String>,
    upstream_addr: String,
    basic_auth: Option<String>,
    host_header: Option<String>,
    upstream_tls: bool,
}

/// Active WebSocket connection to local server
struct LocalWebSocket {
    write: Arc<
        Mutex<
            futures_util::stream::SplitSink<
                WebSocketStream<MaybeTlsStream<TcpStream>>,
                tungstenite::Message,
            >,
        >,
    >,
}

struct RequestBodyState {
    sender: mpsc::Sender<Vec<u8>>,
    last_activity: Instant,
}

impl TunnelClient {
    pub fn new(
        server_url: &str,
        token: &str,
        requested_subdomain: Option<String>,
        upstream_addr: String,
    ) -> Self {
        Self {
            server_url: server_url.to_string(),
            token: token.to_string(),
            requested_subdomain,
            upstream_addr,
            basic_auth: None,
            host_header: None,
            upstream_tls: false,
        }
    }

    pub fn set_basic_auth(&mut self, auth: &str) {
        self.basic_auth = Some(auth.to_string());
    }

    pub fn set_host_header(&mut self, host: &str) {
        self.host_header = Some(host.to_string());
    }

    pub fn set_upstream_tls(&mut self, tls: bool) {
        self.upstream_tls = tls;
    }

    /// Run the tunnel client
    pub async fn run(&mut self) -> Result<()> {
        use cliclack::{intro, note, outro_cancel};

        let url = format!("{}/_dvaar/tunnel", self.server_url);

        intro(style(" dvaar ").on_cyan().black().to_string())?;

        let spinner = cliclack::spinner();
        spinner.start("Connecting to tunnel server...");

        let (ws_stream, _) = connect_async(&url)
            .await
            .context("Failed to connect to tunnel server")?;

        spinner.stop("Connected to server");

        let (mut write, mut read) = ws_stream.split();

        // Send Init packet
        let init = ClientHello {
            token: self.token.clone(),
            requested_subdomain: self.requested_subdomain.clone(),
            tunnel_type: TunnelType::Http,
            client_version: env!("CARGO_PKG_VERSION").to_string(),
        };

        let init_packet = ControlPacket::Init(init);
        let init_bytes = init_packet.to_bytes()?;
        write.send(Message::Binary(init_bytes.into())).await?;

        // Wait for InitAck
        let ack_msg = tokio::time::timeout(Duration::from_secs(10), read.next())
            .await
            .context("Timeout waiting for server response")?
            .ok_or_else(|| anyhow::anyhow!("Connection closed before response"))?
            .context("WebSocket error")?;

        let ack_data = match ack_msg {
            Message::Binary(data) => data,
            _ => anyhow::bail!("Unexpected message type from server"),
        };

        let ack_packet = ControlPacket::from_bytes(&ack_data)?;
        let server_hello = match ack_packet {
            ControlPacket::InitAck(hello) => hello,
            _ => anyhow::bail!("Expected InitAck packet"),
        };

        if let Some(error) = server_hello.error {
            outro_cancel(format!("Server error: {}", error))?;
            anyhow::bail!("Server error: {}", error);
        }

        // Display tunnel info
        let public_url = format!("https://{}", server_hello.assigned_domain);
        let tunnel_info = format!(
            "{} {} {}\n{} {} {}",
            style("Public URL:").dim(),
            style(&public_url).green().bold(),
            style("").dim(),
            style("Forwarding:").dim(),
            style(self.format_upstream()).cyan(),
            style("").dim(),
        );
        note("Tunnel Active", &tunnel_info)?;

        println!();
        println!(
            "{}  {}",
            style("◆").green(),
            style("Waiting for requests... (Ctrl+C to stop)").dim()
        );
        println!();

        // Start bidirectional communication
        self.handle_tunnel(write, read).await
    }

    fn format_upstream(&self) -> String {
        let scheme = if self.upstream_tls { "https" } else { "http" };
        format!("{}://{}", scheme, self.upstream_addr)
    }

    async fn handle_tunnel(
        &self,
        write: futures_util::stream::SplitSink<
            WebSocketStream<MaybeTlsStream<TcpStream>>,
            Message,
        >,
        mut read: futures_util::stream::SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    ) -> Result<()> {
        let write = Arc::new(Mutex::new(write));

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .context("Failed to build HTTP client")?;

        // Track active WebSocket connections for passthrough
        let websockets: Arc<Mutex<HashMap<String, LocalWebSocket>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Channel for sending packets back to server
        let (packet_tx, mut packet_rx) = mpsc::channel::<ControlPacket>(100);

        let upstream_addr = self.upstream_addr.clone();
        let upstream_tls = self.upstream_tls;
        let basic_auth = self.basic_auth.clone();
        let host_header = self.host_header.clone();

        // Ping task
        let ping_tx = packet_tx.clone();
        let ping_task = tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(constants::WS_PING_INTERVAL_SECONDS));
            loop {
                interval.tick().await;
                if ping_tx.send(ControlPacket::Ping).await.is_err() {
                    break;
                }
            }
        });

        // Packet sender task
        let write_clone = write.clone();
        let sender_task = tokio::spawn(async move {
            while let Some(packet) = packet_rx.recv().await {
                let bytes = match packet.to_bytes() {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::error!("Failed to serialize packet: {}", e);
                        continue;
                    }
                };
                let mut w = write_clone.lock().await;
                if w.send(Message::Binary(bytes.into())).await.is_err() {
                    break;
                }
            }
        });

        // Active request body channels (stream_id -> sender + last activity)
        let request_bodies: Arc<Mutex<HashMap<String, RequestBodyState>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let request_bodies_cleanup = request_bodies.clone();
        let cleanup_task = tokio::spawn(async move {
            const REQUEST_BODY_IDLE_TIMEOUT: Duration = Duration::from_secs(300);
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                let cutoff = Instant::now()
                    .checked_sub(REQUEST_BODY_IDLE_TIMEOUT)
                    .unwrap_or_else(Instant::now);
                let mut bodies = request_bodies_cleanup.lock().await;
                bodies.retain(|stream_id, state| {
                    if state.last_activity < cutoff {
                        tracing::warn!("Dropping stale request body stream {}", stream_id);
                        false
                    } else {
                        true
                    }
                });
            }
        });

        loop {
            let msg = match read.next().await {
                Some(Ok(msg)) => msg,
                Some(Err(e)) => {
                    tracing::error!("WebSocket error: {}", e);
                    break;
                }
                None => break,
            };

            match msg {
                Message::Binary(data) => {
                    let packet = match ControlPacket::from_bytes(&data) {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::warn!("Failed to parse packet: {}", e);
                            continue;
                        }
                    };

                    match packet {
                        ControlPacket::HttpRequest(request) => {
                            let stream_id = request.stream_id.clone();
                            let (body_tx, body_rx) = mpsc::channel::<Vec<u8>>(32);
                            request_bodies.lock().await.insert(
                                stream_id,
                                RequestBodyState {
                                    sender: body_tx,
                                    last_activity: Instant::now(),
                                },
                            );

                            let packet_tx = packet_tx.clone();
                            let upstream_addr = upstream_addr.clone();
                            let host_header = host_header.clone();
                            let basic_auth = basic_auth.clone();
                            let websockets = websockets.clone();
                            let http_client = http_client.clone();

                            tokio::spawn(async move {
                                Self::handle_request(
                                    request,
                                    body_rx,
                                    http_client,
                                    &upstream_addr,
                                    upstream_tls,
                                    basic_auth.as_deref(),
                                    host_header.as_deref(),
                                    packet_tx,
                                    websockets,
                                )
                                .await;
                            });
                        }

                        ControlPacket::Data { stream_id, data } => {
                            let body_tx = {
                                let mut bodies = request_bodies.lock().await;
                                if let Some(state) = bodies.get_mut(&stream_id) {
                                    state.last_activity = Instant::now();
                                    Some(state.sender.clone())
                                } else {
                                    None
                                }
                            };

                            if let Some(body_tx) = body_tx {
                                if body_tx.send(data).await.is_err() {
                                    request_bodies.lock().await.remove(&stream_id);
                                }
                            }
                        }

                        ControlPacket::End { stream_id } => {
                            request_bodies.lock().await.remove(&stream_id);
                        }

                        ControlPacket::WebSocketFrame {
                            stream_id,
                            data,
                            is_binary,
                        } => {
                            let ws_sender = {
                                let ws_map = websockets.lock().await;
                                ws_map.get(&stream_id).map(|ws| ws.write.clone())
                            };

                            if let Some(ws_sender) = ws_sender {
                                let msg = if is_binary {
                                    Message::Binary(data.into())
                                } else {
                                    Message::Text(
                                        String::from_utf8_lossy(&data).to_string().into(),
                                    )
                                };

                                let mut ws_sender = ws_sender.lock().await;
                                if ws_sender.send(msg).await.is_err() {
                                    websockets.lock().await.remove(&stream_id);
                                    let _ = packet_tx
                                        .send(ControlPacket::WebSocketClose {
                                            stream_id,
                                            code: Some(1006),
                                            reason: Some("Local connection closed".to_string()),
                                        })
                                        .await;
                                }
                            }
                        }

                        ControlPacket::WebSocketClose { stream_id, .. } => {
                            let ws_sender = {
                                let ws_map = websockets.lock().await;
                                ws_map.get(&stream_id).map(|ws| ws.write.clone())
                            };
                            if let Some(ws_sender) = ws_sender {
                                let mut ws_sender = ws_sender.lock().await;
                                let _ = ws_sender.send(Message::Close(None)).await;
                            }
                            websockets.lock().await.remove(&stream_id);
                        }

                        ControlPacket::Ping => {
                            let _ = packet_tx.send(ControlPacket::Pong).await;
                        }

                        ControlPacket::Pong => {
                            // Server responded to our ping
                        }

                        _ => {
                            tracing::debug!("Unexpected packet type");
                        }
                    }
                }
                Message::Ping(data) => {
                    let mut w = write.lock().await;
                    let _ = w.send(Message::Pong(data)).await;
                }
                Message::Pong(_) => {}
                Message::Close(_) => {
                    println!("Server closed connection");
                    break;
                }
                _ => {}
            }
        }

        ping_task.abort();
        sender_task.abort();
        cleanup_task.abort();
        request_bodies.lock().await.clear();
        Ok(())
    }

    async fn handle_request(
        request: HttpRequestPacket,
        body_rx: mpsc::Receiver<Vec<u8>>,
        http_client: reqwest::Client,
        upstream_addr: &str,
        upstream_tls: bool,
        basic_auth: Option<&str>,
        host_header: Option<&str>,
        packet_tx: mpsc::Sender<ControlPacket>,
        websockets: Arc<Mutex<HashMap<String, LocalWebSocket>>>,
    ) {
        let start_time = Instant::now();
        let stream_id = request.stream_id.clone();
        let method = request.method.clone();
        let uri = request.uri.clone();

        // Check if this is a WebSocket upgrade request
        if request.is_websocket_upgrade() {
            Self::handle_websocket_upgrade(
                request,
                upstream_addr,
                upstream_tls,
                host_header,
                packet_tx,
                websockets,
            )
            .await;
            return;
        }

        // Regular HTTP request
        let scheme = if upstream_tls { "https" } else { "http" };
        let url = format!("{}://{}{}", scheme, upstream_addr, &uri);

        tracing::debug!("{} {}", method, url);

        // Build request using reqwest for streaming support
        let http_method = match method.as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "DELETE" => reqwest::Method::DELETE,
            "PATCH" => reqwest::Method::PATCH,
            "HEAD" => reqwest::Method::HEAD,
            "OPTIONS" => reqwest::Method::OPTIONS,
            _ => reqwest::Method::GET,
        };

        let mut req_builder = http_client.request(http_method, &url);

        // Add headers
        for (key, value) in &request.headers {
            let key_lower = key.to_lowercase();
            // Skip hop-by-hop headers but keep upgrade-related ones
            if key_lower == "host"
                || key_lower == "transfer-encoding"
                || key_lower == "content-length"
            {
                continue;
            }
            req_builder = req_builder.header(key.as_str(), value.as_str());
        }

        // Override host header if specified
        if let Some(host) = host_header {
            req_builder = req_builder.header("Host", host);
        }

        // Add basic auth check
        if let Some(_auth) = basic_auth {
            let has_auth = request
                .headers
                .iter()
                .any(|(k, _)| k.to_lowercase() == "authorization");
            if !has_auth {
                // Return 401
                let response = HttpResponsePacket {
                    stream_id: stream_id.clone(),
                    status: 401,
                    headers: vec![(
                        "WWW-Authenticate".to_string(),
                        "Basic realm=\"dvaar\"".to_string(),
                    )],
                };
                let _ = packet_tx.send(ControlPacket::HttpResponse(response)).await;
                let _ = packet_tx
                    .send(ControlPacket::Data {
                        stream_id: stream_id.clone(),
                        data: b"Unauthorized".to_vec(),
                    })
                    .await;
                let _ = packet_tx.send(ControlPacket::End { stream_id }).await;
                return;
            }
        }

        let body_stream = futures_util::stream::unfold(body_rx, |mut rx| async {
            match rx.recv().await {
                Some(chunk) => Some((Ok::<Bytes, std::io::Error>(Bytes::from(chunk)), rx)),
                None => None,
            }
        });

        req_builder = req_builder.body(reqwest::Body::wrap_stream(body_stream));

        // Send request and stream response
        match req_builder.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let headers: Vec<(String, String)> = response
                    .headers()
                    .iter()
                    .filter_map(|(k, v)| {
                        let key = k.to_string();
                        let key_lower = key.to_lowercase();
                        // Skip hop-by-hop headers
                        if key_lower == "transfer-encoding" || key_lower == "connection" {
                            return None;
                        }
                        v.to_str().ok().map(|s| (key, s.to_string()))
                    })
                    .collect();

                // Send response headers
                let response_packet = HttpResponsePacket {
                    stream_id: stream_id.clone(),
                    status,
                    headers,
                };
                if packet_tx
                    .send(ControlPacket::HttpResponse(response_packet))
                    .await
                    .is_err()
                {
                    return;
                }

                // Stream response body
                let mut total_bytes = 0usize;
                let mut stream = response.bytes_stream();

                while let Some(chunk_result) = stream.next().await {
                    match chunk_result {
                        Ok(chunk) => {
                            total_bytes += chunk.len();
                            // Send in smaller chunks if needed
                            for subchunk in chunk.chunks(STREAM_CHUNK_SIZE) {
                                if packet_tx
                                    .send(ControlPacket::Data {
                                        stream_id: stream_id.clone(),
                                        data: subchunk.to_vec(),
                                    })
                                    .await
                                    .is_err()
                                {
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Error streaming response: {}", e);
                            let _ = packet_tx
                                .send(ControlPacket::StreamError {
                                    stream_id: stream_id.clone(),
                                    error: e.to_string(),
                                })
                                .await;
                            return;
                        }
                    }
                }

                // Send end of stream
                let _ = packet_tx.send(ControlPacket::End { stream_id: stream_id.clone() }).await;

                let elapsed = start_time.elapsed();
                Self::log_request(&method, &uri, status, elapsed, total_bytes);
            }
            Err(e) => {
                tracing::error!("Upstream request failed: {}", e);

                let response = HttpResponsePacket {
                    stream_id: stream_id.clone(),
                    status: 502,
                    headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
                };
                let _ = packet_tx.send(ControlPacket::HttpResponse(response)).await;
                let _ = packet_tx
                    .send(ControlPacket::Data {
                        stream_id: stream_id.clone(),
                        data: format!("Bad Gateway: {}", e).into_bytes(),
                    })
                    .await;
                let _ = packet_tx.send(ControlPacket::End { stream_id: stream_id.clone() }).await;

                let elapsed = start_time.elapsed();
                Self::log_request(&method, &uri, 502, elapsed, 0);
            }
        }
    }

    async fn handle_websocket_upgrade(
        request: HttpRequestPacket,
        upstream_addr: &str,
        upstream_tls: bool,
        host_header: Option<&str>,
        packet_tx: mpsc::Sender<ControlPacket>,
        websockets: Arc<Mutex<HashMap<String, LocalWebSocket>>>,
    ) {
        let stream_id = request.stream_id.clone();
        let scheme = if upstream_tls { "wss" } else { "ws" };
        let url = format!("{}://{}{}", scheme, upstream_addr, request.uri);

        tracing::debug!("WebSocket upgrade: {}", url);

        // Build WebSocket request with original headers
        let mut ws_request = tungstenite::http::Request::builder()
            .uri(&url)
            .method("GET");

        for (key, value) in &request.headers {
            let key_lower = key.to_lowercase();
            if key_lower == "host" && host_header.is_some() {
                continue;
            }
            ws_request = ws_request.header(key.as_str(), value.as_str());
        }

        if let Some(host) = host_header {
            ws_request = ws_request.header("Host", host);
        }

        let ws_request = match ws_request.body(()) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Failed to build WebSocket request: {}", e);
                let response = HttpResponsePacket {
                    stream_id: stream_id.clone(),
                    status: 502,
                    headers: vec![],
                };
                let _ = packet_tx.send(ControlPacket::HttpResponse(response)).await;
                let _ = packet_tx
                    .send(ControlPacket::Data {
                        stream_id: stream_id.clone(),
                        data: format!("Failed to build WebSocket request: {}", e).into_bytes(),
                    })
                    .await;
                let _ = packet_tx.send(ControlPacket::End { stream_id }).await;
                return;
            }
        };

        // Connect to local WebSocket
        match connect_async(ws_request).await {
            Ok((ws_stream, response)) => {
                let status = response.status().as_u16();
                let headers: Vec<(String, String)> = response
                    .headers()
                    .iter()
                    .filter_map(|(k, v)| v.to_str().ok().map(|s| (k.to_string(), s.to_string())))
                    .collect();

                // Send upgrade response to server
                let response_packet = HttpResponsePacket {
                    stream_id: stream_id.clone(),
                    status,
                    headers,
                };
                if packet_tx
                    .send(ControlPacket::HttpResponse(response_packet))
                    .await
                    .is_err()
                {
                    return;
                }

                // No body for upgrade response
                let _ = packet_tx.send(ControlPacket::End { stream_id: stream_id.clone() }).await;

                if status == 101 {
                    // Successfully upgraded, start forwarding frames
                    let (write, mut read) = ws_stream.split();
                    let write = Arc::new(Mutex::new(write));

                    // Store the write half
                    websockets.lock().await.insert(
                        stream_id.clone(),
                        LocalWebSocket { write: write.clone() },
                    );

                    // Spawn task to read from local WebSocket and forward to server
                    let packet_tx = packet_tx.clone();
                    let stream_id_clone = stream_id.clone();
                    let websockets_clone = websockets.clone();
                    let write_for_ping = write.clone();

                    tokio::spawn(async move {
                        while let Some(msg_result) = read.next().await {
                            match msg_result {
                                Ok(msg) => {
                                    let packet = match msg {
                                        Message::Binary(data) => ControlPacket::WebSocketFrame {
                                            stream_id: stream_id_clone.clone(),
                                            data: data.to_vec(),
                                            is_binary: true,
                                        },
                                        Message::Text(text) => ControlPacket::WebSocketFrame {
                                            stream_id: stream_id_clone.clone(),
                                            data: text.as_bytes().to_vec(),
                                            is_binary: false,
                                        },
                                        Message::Ping(data) => {
                                            let mut ws_write = write_for_ping.lock().await;
                                            let _ = ws_write.send(Message::Pong(data)).await;
                                            continue;
                                        }
                                        Message::Pong(_) => continue,
                                        Message::Close(frame) => {
                                            let (code, reason) = frame
                                                .map(|f| (Some(f.code.into()), Some(f.reason.to_string())))
                                                .unwrap_or((None, None));
                                            let _ = packet_tx
                                                .send(ControlPacket::WebSocketClose {
                                                    stream_id: stream_id_clone.clone(),
                                                    code,
                                                    reason,
                                                })
                                                .await;
                                            break;
                                        }
                                        Message::Frame(_) => continue,
                                    };
                                    if packet_tx.send(packet).await.is_err() {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    tracing::debug!("Local WebSocket error: {}", e);
                                    let _ = packet_tx
                                        .send(ControlPacket::WebSocketClose {
                                            stream_id: stream_id_clone.clone(),
                                            code: Some(1006),
                                            reason: Some(e.to_string()),
                                        })
                                        .await;
                                    break;
                                }
                            }
                        }
                        websockets_clone.lock().await.remove(&stream_id_clone);
                    });

                    println!(
                        "  {} {} {} {}",
                        style(chrono::Local::now().format("%H:%M:%S").to_string()).dim(),
                        style("     WS").magenta(),
                        style(&request.uri).white(),
                        style("101").green(),
                    );
                }
            }
            Err(e) => {
                tracing::error!("Failed to connect to local WebSocket: {}", e);
                let response = HttpResponsePacket {
                    stream_id: stream_id.clone(),
                    status: 502,
                    headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
                };
                let _ = packet_tx.send(ControlPacket::HttpResponse(response)).await;
                let _ = packet_tx
                    .send(ControlPacket::Data {
                        stream_id: stream_id.clone(),
                        data: format!("WebSocket connection failed: {}", e).into_bytes(),
                    })
                    .await;
                let _ = packet_tx.send(ControlPacket::End { stream_id }).await;
            }
        }
    }

    /// Pretty print a request log line
    fn log_request(method: &str, uri: &str, status: u16, elapsed: Duration, body_size: usize) {
        use chrono::Local;

        let now = Local::now();
        let timestamp = style(now.format("%H:%M:%S").to_string()).dim();

        // Method styling
        let method_styled = match method {
            "GET" => style(format!("{:>7}", method)).green(),
            "POST" => style(format!("{:>7}", method)).yellow(),
            "PUT" => style(format!("{:>7}", method)).blue(),
            "PATCH" => style(format!("{:>7}", method)).magenta(),
            "DELETE" => style(format!("{:>7}", method)).red(),
            "HEAD" => style(format!("{:>7}", method)).cyan(),
            "OPTIONS" => style(format!("{:>7}", method)).white(),
            _ => style(format!("{:>7}", method)).white(),
        };

        // Status code styling
        let status_styled = if status >= 500 {
            style(status.to_string()).red().bold()
        } else if status >= 400 {
            style(status.to_string()).yellow()
        } else if status >= 300 {
            style(status.to_string()).cyan()
        } else if status >= 200 {
            style(status.to_string()).green()
        } else {
            style(status.to_string()).white()
        };

        // Duration styling
        let elapsed_ms = elapsed.as_millis();
        let duration_styled = if elapsed_ms > 1000 {
            style(format!("{:>6}ms", elapsed_ms)).red()
        } else if elapsed_ms > 500 {
            style(format!("{:>6}ms", elapsed_ms)).yellow()
        } else if elapsed_ms > 100 {
            style(format!("{:>6}ms", elapsed_ms)).white()
        } else {
            style(format!("{:>6}ms", elapsed_ms)).green()
        };

        // Size formatting
        let size_str = if body_size >= 1_000_000 {
            format!("{:.1}MB", body_size as f64 / 1_000_000.0)
        } else if body_size >= 1_000 {
            format!("{:.1}KB", body_size as f64 / 1_000.0)
        } else {
            format!("{}B", body_size)
        };
        let size_styled = style(format!("{:>8}", size_str)).dim();

        // Truncate URI if too long
        let max_uri_len = 50;
        let uri_display = if uri.len() > max_uri_len {
            format!("{}...", &uri[..max_uri_len - 3])
        } else {
            uri.to_string()
        };

        println!(
            "  {} {} {} {} {} {}",
            timestamp, method_styled, style(uri_display).white(), status_styled, duration_styled, size_styled,
        );
    }
}
