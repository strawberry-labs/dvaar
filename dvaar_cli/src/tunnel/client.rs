//! WebSocket tunnel client

use anyhow::{Context, Result};
use dvaar_common::{
    constants, ClientHello, ControlPacket, HttpRequestPacket, HttpResponsePacket, TunnelType,
};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::Message,
    MaybeTlsStream, WebSocketStream,
};

/// Tunnel client for HTTP tunneling
pub struct TunnelClient {
    server_url: String,
    token: String,
    requested_subdomain: Option<String>,
    upstream_addr: String,
    basic_auth: Option<String>,
    host_header: Option<String>,
    upstream_tls: bool,
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
        let url = format!("{}/_dvaar/tunnel", self.server_url);

        println!("Connecting to {}...", url);

        let (ws_stream, _) = connect_async(&url)
            .await
            .context("Failed to connect to tunnel server")?;

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

        println!();
        println!("Tunnel established!");
        println!();
        println!("Public URL:     https://{}", server_hello.assigned_domain);
        println!("Forwarding to:  {}", self.format_upstream());
        println!();
        println!("Press Ctrl+C to stop");
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
        mut write: futures_util::stream::SplitSink<
            WebSocketStream<MaybeTlsStream<TcpStream>>,
            Message,
        >,
        mut read: futures_util::stream::SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    ) -> Result<()> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?;

        let upstream_addr = self.upstream_addr.clone();
        let upstream_tls = self.upstream_tls;
        let basic_auth = self.basic_auth.clone();
        let host_header = self.host_header.clone();

        // Ping task
        let (ping_tx, mut ping_rx) = tokio::sync::mpsc::channel::<()>(1);

        let ping_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(constants::WS_PING_INTERVAL_SECONDS));
            loop {
                interval.tick().await;
                if ping_tx.send(()).await.is_err() {
                    break;
                }
            }
        });

        loop {
            tokio::select! {
                // Send ping
                _ = ping_rx.recv() => {
                    let ping = ControlPacket::Ping.to_bytes()?;
                    if write.send(Message::Binary(ping.into())).await.is_err() {
                        break;
                    }
                }

                // Receive messages
                msg = read.next() => {
                    let msg = match msg {
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
                                    let response = self.forward_request(
                                        &http_client,
                                        request,
                                        &upstream_addr,
                                        upstream_tls,
                                        basic_auth.as_deref(),
                                        host_header.as_deref(),
                                    ).await;

                                    let response_packet = ControlPacket::HttpResponse(response);
                                    let response_bytes = response_packet.to_bytes()?;
                                    write.send(Message::Binary(response_bytes.into())).await?;
                                }
                                ControlPacket::Ping => {
                                    let pong = ControlPacket::Pong.to_bytes()?;
                                    write.send(Message::Binary(pong.into())).await?;
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
                            write.send(Message::Pong(data)).await?;
                        }
                        Message::Pong(_) => {}
                        Message::Close(_) => {
                            println!("Server closed connection");
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        ping_task.abort();
        Ok(())
    }

    async fn forward_request(
        &self,
        client: &reqwest::Client,
        request: HttpRequestPacket,
        upstream_addr: &str,
        upstream_tls: bool,
        basic_auth: Option<&str>,
        host_header: Option<&str>,
    ) -> HttpResponsePacket {
        let scheme = if upstream_tls { "https" } else { "http" };
        let url = format!("{}://{}{}", scheme, upstream_addr, request.uri);

        tracing::info!("{} {}", request.method, url);

        let method = reqwest::Method::from_bytes(request.method.as_bytes())
            .unwrap_or(reqwest::Method::GET);

        let mut req_builder = client.request(method, &url);

        // Add headers
        for (key, value) in &request.headers {
            let key_lower = key.to_lowercase();
            // Skip hop-by-hop headers
            if key_lower == "host" || key_lower == "connection" || key_lower == "transfer-encoding" {
                continue;
            }
            req_builder = req_builder.header(key.as_str(), value.as_str());
        }

        // Override host header if specified
        if let Some(host) = host_header {
            req_builder = req_builder.header("Host", host);
        }

        // Add basic auth if configured
        if let Some(_auth) = basic_auth {
            // Check if request has matching auth
            let has_auth = request.headers.iter().any(|(k, _)| k.to_lowercase() == "authorization");
            if !has_auth {
                // Return 401
                return HttpResponsePacket {
                    stream_id: request.stream_id,
                    status: 401,
                    headers: vec![
                        ("WWW-Authenticate".to_string(), "Basic realm=\"dvaar\"".to_string()),
                    ],
                    body: b"Unauthorized".to_vec(),
                };
            }
        }

        // Add body
        if !request.body.is_empty() {
            req_builder = req_builder.body(request.body.clone());
        }

        // Send request
        match req_builder.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let headers: Vec<(String, String)> = response
                    .headers()
                    .iter()
                    .filter_map(|(k, v)| {
                        v.to_str().ok().map(|s| (k.to_string(), s.to_string()))
                    })
                    .collect();

                let body = response.bytes().await.unwrap_or_default().to_vec();

                HttpResponsePacket {
                    stream_id: request.stream_id,
                    status,
                    headers,
                    body,
                }
            }
            Err(e) => {
                tracing::error!("Upstream request failed: {}", e);
                HttpResponsePacket {
                    stream_id: request.stream_id,
                    status: 502,
                    headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
                    body: format!("Bad Gateway: {}", e).into_bytes(),
                }
            }
        }
    }
}
