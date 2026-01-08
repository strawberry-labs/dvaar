//! WebSocket tunnel client with streaming and WebSocket passthrough support

use crate::inspector::{CapturedRequest, InspectorClient, RequestStore};
use crate::tui::{TuiApp, TuiEvent, TunnelInfo, TunnelStatus};
use anyhow::{Context, Result};
use bytes::Bytes;
use chrono::Utc;
use console::style;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dvaar_common::{
    constants, ClientHello, ControlPacket, HttpRequestPacket, HttpResponsePacket, TunnelType,
};
use futures_util::{SinkExt, StreamExt};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::collections::HashMap;
use std::io;
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
    inspector: Option<Arc<RequestStore>>,
    inspector_client: Option<Arc<InspectorClient>>,
    tunnel_id: Option<String>,
    user_email: Option<String>,
    user_plan: Option<String>,
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
            inspector: None,
            inspector_client: None,
            tunnel_id: None,
            user_email: None,
            user_plan: None,
        }
    }

    pub fn set_user_info(&mut self, email: Option<String>, plan: Option<String>) {
        self.user_email = email;
        self.user_plan = plan;
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

    pub fn set_inspector(&mut self, store: Arc<RequestStore>) {
        self.inspector = Some(store);
    }

    pub fn set_inspector_client(&mut self, client: InspectorClient) {
        self.inspector_client = Some(Arc::new(client));
    }

    pub fn set_tunnel_id(&mut self, id: String) {
        self.tunnel_id = Some(id);
    }

    /// Run the tunnel client
    pub async fn run(&mut self, inspect_port: Option<u16>, tui_mode: bool) -> Result<()> {
        if tui_mode {
            self.run_with_tui(inspect_port).await
        } else {
            self.run_simple(inspect_port).await
        }
    }

    /// Run with simple CLI output (original behavior)
    async fn run_simple(&mut self, inspect_port: Option<u16>) -> Result<()> {
        use cliclack::{intro, note, outro_cancel};

        let url = format!("{}/_dvaar/tunnel", self.server_url);

        intro(style(" dvaar ").on_cyan().black().to_string())?;

        let spinner = cliclack::spinner();
        spinner.start("Connecting to tunnel server...");

        let start_time = Instant::now();
        let (ws_stream, _) = connect_async(&url)
            .await
            .context("Failed to connect to tunnel server")?;
        let latency_ms = start_time.elapsed().as_millis() as u64;

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

        // Display tunnel info with clickable links
        let public_url = format!("https://{}", server_hello.assigned_domain);
        let upstream_url = self.format_upstream();

        // Update tunnel info in inspector store (server mode)
        // This updates both the legacy tunnel_info and the registered tunnel's public_url
        let server_heartbeat_task = if let Some(ref store) = self.inspector {
            if let Some(ref tunnel_id) = self.tunnel_id {
                // Update the registered tunnel's public_url
                store.update_tunnel_url(tunnel_id, public_url.clone()).await;
                // Also update local_addr in legacy info
                store.set_tunnel_info(public_url.clone(), upstream_url.clone()).await;

                // Start heartbeat task for server-mode tunnel
                let store_clone = Arc::clone(store);
                let tunnel_id_clone = tunnel_id.clone();
                Some(tokio::spawn(async move {
                    let mut interval = tokio::time::interval(Duration::from_secs(30));
                    loop {
                        interval.tick().await;
                        store_clone.heartbeat(&tunnel_id_clone).await;
                    }
                }))
            } else {
                store.set_tunnel_info(public_url.clone(), upstream_url.clone()).await;
                None
            }
        } else {
            None
        };

        // Register with inspector client if in client mode and start heartbeat
        let client_heartbeat_task = if let Some(ref client) = self.inspector_client {
            if let Err(e) = client.register(
                &self.requested_subdomain.clone().unwrap_or_default(),
                &public_url,
                &upstream_url,
            ).await {
                tracing::warn!("Failed to register with inspector: {}", e);
                None
            } else {
                // Start heartbeat task to keep registration alive
                Some(Arc::clone(client).start_heartbeat_task())
            }
        } else {
            None
        };

        let mut tunnel_info = format!(
            "{} {} {}\n{} {} {}",
            style("Public URL:").dim(),
            style(terminal_link(&public_url, &public_url)).green().bold(),
            style("").dim(),
            style("Forwarding:").dim(),
            style(terminal_link(&upstream_url, &upstream_url)).cyan(),
            style("").dim(),
        );

        // Add inspector URL if enabled
        if let Some(port) = inspect_port {
            let inspector_url = format!("http://localhost:{}", port);
            tunnel_info.push_str(&format!(
                "\n{} {} {}",
                style("Inspector:").dim(),
                style(terminal_link(&inspector_url, &inspector_url)).magenta().bold(),
                style("").dim(),
            ));
        }

        // Add latency info
        tunnel_info.push_str(&format!(
            "\n{} {}",
            style("Latency:").dim(),
            style(format!("{}ms", latency_ms)).white(),
        ));

        note("Tunnel Active", &tunnel_info)?;

        // Display QR code
        print_qr_code(&public_url);

        println!();
        println!(
            "{}  {}",
            style("◆").green(),
            style("Waiting for requests... (Ctrl+C to stop)").dim()
        );
        println!();

        // Start bidirectional communication
        let result = self.handle_tunnel(write, read, None).await;

        // Cleanup: abort heartbeat tasks
        if let Some(task) = server_heartbeat_task {
            task.abort();
        }
        if let Some(task) = client_heartbeat_task {
            task.abort();
        }

        // Unregister from inspector on shutdown
        if let Some(ref client) = self.inspector_client {
            let _ = client.unregister().await;
        }

        result
    }

    /// Run with full TUI
    async fn run_with_tui(&mut self, inspect_port: Option<u16>) -> Result<()> {
        let url = format!("{}/_dvaar/tunnel", self.server_url);

        // Measure connection latency
        let start_time = Instant::now();
        let (ws_stream, _) = connect_async(&url)
            .await
            .context("Failed to connect to tunnel server")?;
        let latency_ms = start_time.elapsed().as_millis() as u64;

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
            anyhow::bail!("Server error: {}", error);
        }

        let public_url = format!("https://{}", server_hello.assigned_domain);
        let local_addr = self.format_upstream();
        let inspector_url = inspect_port.map(|p| format!("http://localhost:{}", p));

        // Update tunnel info in inspector store (server mode)
        // This updates both the legacy tunnel_info and the registered tunnel's public_url
        let server_heartbeat_task = if let Some(ref store) = self.inspector {
            if let Some(ref tunnel_id) = self.tunnel_id {
                // Update the registered tunnel's public_url
                store.update_tunnel_url(tunnel_id, public_url.clone()).await;
                // Also update local_addr in legacy info
                store.set_tunnel_info(public_url.clone(), local_addr.clone()).await;

                // Start heartbeat task for server-mode tunnel
                let store_clone = Arc::clone(store);
                let tunnel_id_clone = tunnel_id.clone();
                Some(tokio::spawn(async move {
                    let mut interval = tokio::time::interval(Duration::from_secs(30));
                    loop {
                        interval.tick().await;
                        store_clone.heartbeat(&tunnel_id_clone).await;
                    }
                }))
            } else {
                store.set_tunnel_info(public_url.clone(), local_addr.clone()).await;
                None
            }
        } else {
            None
        };

        // Register with inspector client if in client mode and start heartbeat
        let client_heartbeat_task = if let Some(ref client) = self.inspector_client {
            if let Err(e) = client.register(
                &self.requested_subdomain.clone().unwrap_or_default(),
                &public_url,
                &local_addr,
            ).await {
                tracing::warn!("Failed to register with inspector: {}", e);
                None
            } else {
                // Start heartbeat task to keep registration alive
                Some(Arc::clone(client).start_heartbeat_task())
            }
        } else {
            None
        };

        // Create TUI app
        let tunnel_info = TunnelInfo {
            public_url: public_url.clone(),
            local_addr,
            inspector_url,
            status: TunnelStatus::Online,
            user_email: self.user_email.clone(),
            user_plan: self.user_plan.clone(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            latency_ms: Some(latency_ms),
        };

        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Create channel for TUI events
        let (tui_tx, tui_rx) = mpsc::channel::<TuiEvent>(100);

        let mut app = TuiApp::new(tunnel_info);

        // Fetch ads from server in background (don't block TUI startup)
        let server_url = self.server_url.clone();
        let ads_tx = tui_tx.clone();
        tokio::spawn(async move {
            let ads = fetch_ads_from_server(&server_url).await;
            let _ = ads_tx.send(TuiEvent::AdsUpdate(ads)).await;
        });

        // Run event loop
        let result = self
            .run_tui_loop(&mut terminal, &mut app, write, read, tui_tx, tui_rx, server_heartbeat_task, client_heartbeat_task)
            .await;

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen
        )?;
        terminal.show_cursor()?;

        result
    }

    async fn run_tui_loop(
        &self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        app: &mut TuiApp,
        write: futures_util::stream::SplitSink<
            WebSocketStream<MaybeTlsStream<TcpStream>>,
            Message,
        >,
        mut read: futures_util::stream::SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        tui_tx: mpsc::Sender<TuiEvent>,
        mut tui_rx: mpsc::Receiver<TuiEvent>,
        server_heartbeat_task: Option<tokio::task::JoinHandle<()>>,
        client_heartbeat_task: Option<tokio::task::JoinHandle<()>>,
    ) -> Result<()> {
        struct HeartbeatGuard {
            server: Option<tokio::task::JoinHandle<()>>,
            client: Option<tokio::task::JoinHandle<()>>,
        }

        impl HeartbeatGuard {
            fn new(
                server: Option<tokio::task::JoinHandle<()>>,
                client: Option<tokio::task::JoinHandle<()>>,
            ) -> Self {
                Self { server, client }
            }

            fn abort_all(&mut self) {
                if let Some(task) = self.server.take() {
                    task.abort();
                }
                if let Some(task) = self.client.take() {
                    task.abort();
                }
            }
        }

        impl Drop for HeartbeatGuard {
            fn drop(&mut self) {
                self.abort_all();
            }
        }

        let write = Arc::new(Mutex::new(write));
        let (packet_tx, mut packet_rx) = mpsc::channel::<ControlPacket>(100);
        let mut heartbeat_guard = HeartbeatGuard::new(server_heartbeat_task, client_heartbeat_task);

        // Active request body receivers
        let body_receivers: Arc<Mutex<HashMap<String, RequestBodyState>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Active WebSocket connections
        let websockets: Arc<Mutex<HashMap<String, LocalWebSocket>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // HTTP client for upstream requests
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .pool_max_idle_per_host(10)
            .build()?;

        let upstream_addr = self.upstream_addr.clone();
        let upstream_tls = self.upstream_tls;
        let basic_auth = self.basic_auth.clone();
        let host_header = self.host_header.clone();
        let inspector = self.inspector.clone();
        let inspector_client = self.inspector_client.clone();
        let tunnel_id = self.tunnel_id.clone();

        // Metrics update interval
        let mut metrics_interval = tokio::time::interval(Duration::from_secs(1));
        let mut tick_interval = tokio::time::interval(Duration::from_millis(100));
        let mut ping_interval = tokio::time::interval(Duration::from_secs(constants::WS_PING_INTERVAL_SECONDS));
        // Ad rotation starts after 15 seconds (not immediately)
        let mut ad_rotation_interval = tokio::time::interval_at(
            tokio::time::Instant::now() + Duration::from_secs(15),
            Duration::from_secs(15),
        );

        loop {
            // Draw UI
            terminal.draw(|f| crate::tui::draw(f, app))?;

            tokio::select! {
                // Handle keyboard events (non-blocking)
                _ = tick_interval.tick() => {
                    if event::poll(Duration::from_millis(0))? {
                        if let Event::Key(key) = event::read()? {
                            app.handle_event(TuiEvent::Key(key));
                            if app.should_quit {
                                // Cleanup: abort heartbeat tasks
                                heartbeat_guard.abort_all();
                                // Unregister from inspector on quit
                                if let Some(ref client) = self.inspector_client {
                                    let _ = client.unregister().await;
                                }
                                return Ok(());
                            }
                        }
                    }
                }

                // Handle WebSocket messages from server
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Binary(data))) => {
                            match ControlPacket::from_bytes(&data) {
                                Ok(packet) => {
                                    match packet {
                                        ControlPacket::HttpRequest(request) => {
                                            let packet_tx = packet_tx.clone();
                                            let upstream_addr = upstream_addr.clone();
                                            let basic_auth = basic_auth.clone();
                                            let host_header = host_header.clone();
                                            let body_receivers = body_receivers.clone();
                                            let websockets = websockets.clone();
                                            let http_client = http_client.clone();
                                            let inspector = inspector.clone();
                                            let inspector_client = inspector_client.clone();
                                            let tunnel_id = tunnel_id.clone();
                                            let tui_tx = tui_tx.clone();

                                            tokio::spawn(async move {
                                                Self::handle_request_with_tui(
                                                    request,
                                                    upstream_addr,
                                                    upstream_tls,
                                                    basic_auth.as_deref(),
                                                    host_header.as_deref(),
                                                    packet_tx,
                                                    websockets,
                                                    inspector,
                                                    inspector_client,
                                                    tunnel_id,
                                                    http_client,
                                                    body_receivers,
                                                    tui_tx,
                                                )
                                                .await;
                                            });
                                        }
                                        ControlPacket::Data { stream_id, data } => {
                                            let mut receivers = body_receivers.lock().await;
                                            if let Some(state) = receivers.get_mut(&stream_id) {
                                                state.last_activity = Instant::now();
                                                let _ = state.sender.send(data).await;
                                            }
                                        }
                                        ControlPacket::End { stream_id } => {
                                            body_receivers.lock().await.remove(&stream_id);
                                        }
                                        ControlPacket::Ping => {
                                            let _ = packet_tx.send(ControlPacket::Pong).await;
                                        }
                                        ControlPacket::Pong => {
                                            // Server responded to our ping
                                        }
                                        ControlPacket::WebSocketFrame { stream_id, data, is_binary } => {
                                            let ws_sender = {
                                                let ws_map = websockets.lock().await;
                                                ws_map.get(&stream_id).map(|ws| ws.write.clone())
                                            };

                                            if let Some(ws_sender) = ws_sender {
                                                let msg = if is_binary {
                                                    Message::Binary(data.into())
                                                } else {
                                                    Message::Text(String::from_utf8_lossy(&data).to_string().into())
                                                };

                                                let mut ws_sender = ws_sender.lock().await;
                                                if ws_sender.send(msg).await.is_err() {
                                                    websockets.lock().await.remove(&stream_id);
                                                    let _ = packet_tx.send(ControlPacket::WebSocketClose {
                                                        stream_id,
                                                        code: Some(1006),
                                                        reason: Some("Local connection closed".to_string()),
                                                    }).await;
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
                                        _ => {}
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to parse packet: {}", e);
                                }
                            }
                        }
                        Some(Ok(Message::Ping(_))) => {}
                        Some(Ok(Message::Pong(_))) => {}
                        Some(Ok(Message::Close(_))) => {
                            app.tunnel_info.status = TunnelStatus::Offline;
                            // Cleanup: abort heartbeat tasks
                            heartbeat_guard.abort_all();
                            // Unregister from inspector on shutdown
                            if let Some(ref client) = self.inspector_client {
                                let _ = client.unregister().await;
                            }
                            return Ok(());
                        }
                        Some(Err(e)) => {
                            tracing::error!("WebSocket error: {}", e);
                            app.tunnel_info.status = TunnelStatus::Offline;
                            // Cleanup: abort heartbeat tasks
                            heartbeat_guard.abort_all();
                            // Unregister from inspector on shutdown
                            if let Some(ref client) = self.inspector_client {
                                let _ = client.unregister().await;
                            }
                            return Err(e.into());
                        }
                        None => {
                            app.tunnel_info.status = TunnelStatus::Offline;
                            // Cleanup: abort heartbeat tasks
                            heartbeat_guard.abort_all();
                            // Unregister from inspector on shutdown
                            if let Some(ref client) = self.inspector_client {
                                let _ = client.unregister().await;
                            }
                            return Ok(());
                        }
                        _ => {}
                    }
                }

                // Send packets back to server
                Some(packet) = packet_rx.recv() => {
                    let bytes = packet.to_bytes()?;
                    let mut write = write.lock().await;
                    write.send(Message::Binary(bytes.into())).await?;
                }

                // Update metrics periodically
                _ = metrics_interval.tick() => {
                    if let Some(ref store) = self.inspector {
                        // Use tunnel-specific metrics if we have a tunnel_id
                        let metrics = if let Some(ref tid) = self.tunnel_id {
                            store.get_tunnel_metrics(tid).await.unwrap_or_default()
                        } else {
                            store.get_metrics().await
                        };
                        app.update_metrics(metrics);
                    }
                }

                // Send ping to keep connection alive
                _ = ping_interval.tick() => {
                    let _ = packet_tx.send(ControlPacket::Ping).await;
                }

                // Rotate ads periodically
                _ = ad_rotation_interval.tick() => {
                    app.handle_event(TuiEvent::AdRotate);
                }

                // Handle TUI events from request handlers
                Some(event) = tui_rx.recv() => {
                    app.handle_event(event);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_request_with_tui(
        request: HttpRequestPacket,
        upstream_addr: String,
        upstream_tls: bool,
        basic_auth: Option<&str>,
        host_header: Option<&str>,
        packet_tx: mpsc::Sender<ControlPacket>,
        websockets: Arc<Mutex<HashMap<String, LocalWebSocket>>>,
        inspector: Option<Arc<RequestStore>>,
        inspector_client: Option<Arc<InspectorClient>>,
        tunnel_id: Option<String>,
        http_client: reqwest::Client,
        body_receivers: Arc<Mutex<HashMap<String, RequestBodyState>>>,
        tui_tx: mpsc::Sender<TuiEvent>,
    ) {
        // Create body channel for this request
        let (body_tx, body_rx) = mpsc::channel::<Vec<u8>>(100);

        // Register the body receiver
        {
            let mut receivers = body_receivers.lock().await;
            receivers.insert(
                request.stream_id.clone(),
                RequestBodyState {
                    sender: body_tx,
                    last_activity: Instant::now(),
                },
            );
        }

        // Handle the request
        Self::handle_request(
            request,
            body_rx,
            http_client,
            &upstream_addr,
            upstream_tls,
            basic_auth,
            host_header,
            packet_tx,
            websockets,
            inspector,
            inspector_client,
            tunnel_id,
            Some(tui_tx),
        )
        .await;
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
        _tui_tx: Option<mpsc::Sender<TuiEvent>>,
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
        let inspector = self.inspector.clone();
        let inspector_client = self.inspector_client.clone();
        let tunnel_id = self.tunnel_id.clone();

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
                            let inspector = inspector.clone();
                            let inspector_client = inspector_client.clone();
                            let tunnel_id = tunnel_id.clone();

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
                                    inspector,
                                    inspector_client,
                                    tunnel_id,
                                    None, // No TUI in simple mode
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
        inspector: Option<Arc<RequestStore>>,
        inspector_client: Option<Arc<InspectorClient>>,
        tunnel_id: Option<String>,
        tui_tx: Option<mpsc::Sender<TuiEvent>>,
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

        // Increment open connections count
        if let Some(ref store) = inspector {
            if let Some(metrics) = store.metrics_for_tunnel(&tunnel_id.clone().unwrap_or_default()).await {
                metrics.increment_connections().await;
            }
        }
        // Emit connection opened event for TUI (works in both modes)
        if let Some(ref tx) = tui_tx {
            let _ = tx.send(TuiEvent::ConnectionOpened).await;
        }

        // Store request headers for inspector
        let request_headers = request.headers.clone();

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
                // Decrement connection count before early return
                if let Some(ref store) = inspector {
                    if let Some(metrics) = store.metrics_for_tunnel(&tunnel_id.clone().unwrap_or_default()).await {
                        metrics.decrement_connections().await;
                    }
                }
                // Emit connection closed event for TUI
                if let Some(ref tx) = tui_tx {
                    let _ = tx.send(TuiEvent::ConnectionClosed).await;
                }
                return;
            }
        }

        // Collect request body chunks for inspector (if enabled) and create stream
        let capture_body = inspector.is_some() || inspector_client.is_some();
        let mut captured_request_body = Vec::new();

        // Collect all body chunks first
        let mut body_chunks = Vec::new();
        let mut body_rx = body_rx;
        while let Some(chunk) = body_rx.recv().await {
            if capture_body && captured_request_body.len() < 1024 * 1024 {
                captured_request_body.extend_from_slice(&chunk);
            }
            body_chunks.push(chunk);
        }

        // Create stream from collected chunks
        let body_stream = futures_util::stream::iter(
            body_chunks.into_iter().map(|chunk| Ok::<Bytes, std::io::Error>(Bytes::from(chunk)))
        );

        req_builder = req_builder.body(reqwest::Body::wrap_stream(body_stream));

        // Send request and stream response
        match req_builder.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let response_headers: Vec<(String, String)> = response
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
                    headers: response_headers.clone(),
                };
                if packet_tx
                    .send(ControlPacket::HttpResponse(response_packet))
                    .await
                    .is_err()
                {
                    if let Some(ref store) = inspector {
                        if let Some(metrics) = store.metrics_for_tunnel(&tunnel_id.clone().unwrap_or_default()).await {
                            metrics.decrement_connections().await;
                        }
                    }
                    // Emit connection closed event for TUI
                    if let Some(ref tx) = tui_tx {
                        let _ = tx.send(TuiEvent::ConnectionClosed).await;
                    }
                    return;
                }

                // Stream response body and capture for inspector
                let mut total_bytes = 0usize;
                let mut captured_response_body = Vec::new();
                let mut stream = response.bytes_stream();

                while let Some(chunk_result) = stream.next().await {
                    match chunk_result {
                        Ok(chunk) => {
                            total_bytes += chunk.len();

                            // Capture response body (limit to 1MB)
                            if capture_body && captured_response_body.len() < 1024 * 1024 {
                                captured_response_body.extend_from_slice(&chunk);
                            }

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
                                    if let Some(ref store) = inspector {
                                        if let Some(metrics) = store.metrics_for_tunnel(&tunnel_id.clone().unwrap_or_default()).await {
                                            metrics.decrement_connections().await;
                                        }
                                    }
                                    // Emit connection closed event for TUI
                                    if let Some(ref tx) = tui_tx {
                                        let _ = tx.send(TuiEvent::ConnectionClosed).await;
                                    }
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
                            if let Some(ref store) = inspector {
                                if let Some(metrics) = store.metrics_for_tunnel(&tunnel_id.clone().unwrap_or_default()).await {
                                    metrics.decrement_connections().await;
                                }
                            }
                            // Emit connection closed event for TUI
                            if let Some(ref tx) = tui_tx {
                                let _ = tx.send(TuiEvent::ConnectionClosed).await;
                            }
                            return;
                        }
                    }
                }

                // Send end of stream
                let _ = packet_tx.send(ControlPacket::End { stream_id: stream_id.clone() }).await;

                let elapsed = start_time.elapsed();
                Self::log_request(&method, &uri, status, elapsed, total_bytes);

                // Store captured request in inspector and emit to TUI
                if inspector.is_some() || inspector_client.is_some() {
                    let captured = CapturedRequest {
                        id: stream_id.clone(),
                        tunnel_id: tunnel_id.clone().unwrap_or_default(),
                        timestamp: Utc::now(),
                        method: method.clone(),
                        path: uri.clone(),
                        request_headers,
                        request_body: captured_request_body,
                        response_status: status,
                        response_headers,
                        response_body: captured_response_body,
                        duration_ms: elapsed.as_millis() as u64,
                        size_bytes: total_bytes,
                    };
                    // Emit to TUI
                    if let Some(ref tx) = tui_tx {
                        let _ = tx.send(TuiEvent::NewRequest(captured.clone())).await;
                    }
                    // Submit to inspector (client mode) or local store (server mode)
                    if let Some(ref client) = inspector_client {
                        let _ = client.submit_request(captured).await;
                    } else if let Some(ref store) = inspector {
                        store.add_request_for_tunnel(&tunnel_id.clone().unwrap_or_default(), captured).await;
                        // Decrement connection count (server mode only - has local metrics)
                        if let Some(metrics) = store.metrics_for_tunnel(&tunnel_id.clone().unwrap_or_default()).await {
                            metrics.decrement_connections().await;
                        }
                    }
                }
                // Emit connection closed event for TUI (always, even without inspector)
                if let Some(ref tx) = tui_tx {
                    let _ = tx.send(TuiEvent::ConnectionClosed).await;
                }
            }
            Err(e) => {
                tracing::error!("Upstream request failed: {}", e);

                let error_body = format!("Bad Gateway: {}", e).into_bytes();
                let response_headers = vec![("Content-Type".to_string(), "text/plain".to_string())];

                let response = HttpResponsePacket {
                    stream_id: stream_id.clone(),
                    status: 502,
                    headers: response_headers.clone(),
                };
                let _ = packet_tx.send(ControlPacket::HttpResponse(response)).await;
                let _ = packet_tx
                    .send(ControlPacket::Data {
                        stream_id: stream_id.clone(),
                        data: error_body.clone(),
                    })
                    .await;
                let _ = packet_tx.send(ControlPacket::End { stream_id: stream_id.clone() }).await;

                let elapsed = start_time.elapsed();
                Self::log_request(&method, &uri, 502, elapsed, 0);

                // Store failed request in inspector and emit to TUI
                if inspector.is_some() || inspector_client.is_some() {
                    let captured = CapturedRequest {
                        id: stream_id.clone(),
                        tunnel_id: tunnel_id.clone().unwrap_or_default(),
                        timestamp: Utc::now(),
                        method: method.clone(),
                        path: uri.clone(),
                        request_headers,
                        request_body: captured_request_body,
                        response_status: 502,
                        response_headers,
                        response_body: error_body,
                        duration_ms: elapsed.as_millis() as u64,
                        size_bytes: 0,
                    };
                    // Emit to TUI
                    if let Some(ref tx) = tui_tx {
                        let _ = tx.send(TuiEvent::NewRequest(captured.clone())).await;
                    }
                    // Submit to inspector (client mode) or local store (server mode)
                    if let Some(ref client) = inspector_client {
                        let _ = client.submit_request(captured).await;
                    } else if let Some(ref store) = inspector {
                        store.add_request_for_tunnel(&tunnel_id.clone().unwrap_or_default(), captured).await;
                        // Decrement connection count (server mode only - has local metrics)
                        if let Some(metrics) = store.metrics_for_tunnel(&tunnel_id.clone().unwrap_or_default()).await {
                            metrics.decrement_connections().await;
                        }
                    }
                }
                // Emit connection closed event for TUI
                if let Some(ref tx) = tui_tx {
                    let _ = tx.send(TuiEvent::ConnectionClosed).await;
                }
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

/// Print a QR code for the given URL
fn print_qr_code(url: &str) {
    use qrcode::QrCode;

    let code = match QrCode::new(url) {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("Failed to generate QR code: {}", e);
            return;
        }
    };

    let string = code
        .render::<char>()
        .quiet_zone(false)
        .module_dimensions(2, 1)
        .build();

    println!();
    println!("{}", style("  Scan to open:").dim());
    for line in string.lines() {
        println!("  {}", line);
    }
}

/// Create a clickable terminal hyperlink using OSC 8 escape sequence
/// Supported by most modern terminals (iTerm2, Windows Terminal, GNOME Terminal, etc.)
fn terminal_link(url: &str, text: &str) -> String {
    format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, text)
}

/// Fetch ads from the server
async fn fetch_ads_from_server(server_url: &str) -> Vec<crate::tui::Ad> {
    use crate::tui::Ad;

    // Parse URL and extract host, stripping any path (e.g., /_dvaar/tunnel)
    let (scheme, host) = if server_url.starts_with("wss://") {
        let rest = server_url.strip_prefix("wss://").unwrap_or(server_url);
        let host = rest.split('/').next().unwrap_or(rest);
        ("https", host)
    } else if server_url.starts_with("ws://") {
        let rest = server_url.strip_prefix("ws://").unwrap_or(server_url);
        let host = rest.split('/').next().unwrap_or(rest);
        ("http", host)
    } else if server_url.starts_with("https://") {
        let rest = server_url.strip_prefix("https://").unwrap_or(server_url);
        let host = rest.split('/').next().unwrap_or(rest);
        ("https", host)
    } else if server_url.starts_with("http://") {
        let rest = server_url.strip_prefix("http://").unwrap_or(server_url);
        let host = rest.split('/').next().unwrap_or(rest);
        ("http", host)
    } else {
        ("https", server_url)
    };

    // Replace tunnel/api server host with admin server host
    // e.g., tunnel.dvaar.app -> admin.dvaar.app
    //       api.dvaar.io -> admin.dvaar.io
    let admin_host = host
        .replace("tunnel.dvaar.app", "admin.dvaar.app")
        .replace("tunnel.dvaar.io", "admin.dvaar.io")
        .replace("api.dvaar.app", "admin.dvaar.app")
        .replace("api.dvaar.io", "admin.dvaar.io");

    let ads_url = format!("{}://{}/api/ads", scheme, admin_host);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok();

    if let Some(client) = client {
        match client.get(&ads_url).send().await {
            Ok(response) if response.status().is_success() => {
                match response.json::<Vec<Ad>>().await {
                    Ok(ads) if !ads.is_empty() => return ads,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // Return default ads if fetch fails
    vec![
        Ad {
            title: "berrydesk.com".to_string(),
            description: "ai agents for effortless customer support".to_string(),
            url: "https://berrydesk.com".to_string(),
        },
        Ad {
            title: "berrycode.ai".to_string(),
            description: "ai agent orchestration inspired by ralph wiggum".to_string(),
            url: "https://berrycode.ai".to_string(),
        },
    ]
}
