//! TUI application state and event handling

use crate::inspector::CapturedRequest;
use crate::metrics::MetricsSnapshot;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Advertisement/sponsor to display in TUI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ad {
    /// Short title (e.g., "Berry.me")
    pub title: String,
    /// Description (e.g., "AI assistant like Manus")
    pub description: String,
    /// URL to visit
    pub url: String,
}

impl Default for Ad {
    fn default() -> Self {
        Self {
            title: "Berry.me".to_string(),
            description: "AI assistant that does tasks for you".to_string(),
            url: "https://berry.me".to_string(),
        }
    }
}

/// TUI view modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Main,
    RequestList,
}

/// Tunnel connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelStatus {
    Connecting,
    Online,
    Reconnecting,
    Offline,
}

impl TunnelStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TunnelStatus::Connecting => "connecting",
            TunnelStatus::Online => "online",
            TunnelStatus::Reconnecting => "reconnecting",
            TunnelStatus::Offline => "offline",
        }
    }
}

/// Tunnel information for display
#[derive(Debug, Clone)]
pub struct TunnelInfo {
    pub public_url: String,
    pub local_addr: String,
    pub inspector_url: Option<String>,
    pub status: TunnelStatus,
    pub user_email: Option<String>,
    pub user_plan: Option<String>,
    pub version: String,
    pub latency_ms: Option<u64>,
}

impl Default for TunnelInfo {
    fn default() -> Self {
        Self {
            public_url: String::new(),
            local_addr: String::new(),
            inspector_url: None,
            status: TunnelStatus::Connecting,
            user_email: None,
            user_plan: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
            latency_ms: None,
        }
    }
}

/// Events that can be sent to the TUI
#[derive(Debug, Clone)]
pub enum TuiEvent {
    /// New request captured
    NewRequest(CapturedRequest),
    /// Metrics updated
    MetricsUpdate(MetricsSnapshot),
    /// Tunnel status changed
    StatusChange(TunnelStatus),
    /// Tunnel info updated
    TunnelInfoUpdate(TunnelInfo),
    /// Key event from terminal
    Key(KeyEvent),
    /// Tick for periodic updates
    Tick,
    /// Ad rotation tick
    AdRotate,
    /// Update ads list from server
    AdsUpdate(Vec<Ad>),
    /// Connection opened (for tracking open connections in client mode)
    ConnectionOpened,
    /// Connection closed (for tracking open connections in client mode)
    ConnectionClosed,
}

/// TUI application state
pub struct TuiApp {
    pub view: View,
    pub tunnel_info: TunnelInfo,
    pub metrics: MetricsSnapshot,
    pub recent_requests: VecDeque<CapturedRequest>,
    pub all_requests: Vec<CapturedRequest>,
    pub scroll_offset: usize,
    pub selected_index: usize,
    pub should_quit: bool,
    /// QR code as text lines for rendering
    pub qr_code_lines: Vec<String>,
    /// List of ads to rotate through
    pub ads: Vec<Ad>,
    /// Current ad index
    pub current_ad_index: usize,
    /// Local tracking of open connections (for client mode)
    pub local_open_connections: u32,
}

impl TuiApp {
    pub fn new(tunnel_info: TunnelInfo) -> Self {
        // Generate QR code for the public URL
        let qr_code_lines = generate_qr_code(&tunnel_info.public_url);

        // Default ads (will be replaced by server fetch)
        let default_ads = vec![
            Ad {
                title: "Berry.me".to_string(),
                description: "AI assistant that does tasks for you".to_string(),
                url: "https://berry.me".to_string(),
            },
            Ad {
                title: "Ralfie.ai".to_string(),
                description: "Open source AI agent orchestration".to_string(),
                url: "https://ralfie.ai".to_string(),
            },
        ];

        Self {
            view: View::Main,
            tunnel_info,
            metrics: MetricsSnapshot {
                total_requests: 0,
                open_connections: 0,
                requests_per_minute_1m: 0.0,
                requests_per_minute_5m: 0.0,
                requests_per_minute_15m: 0.0,
                p50_duration_ms: 0,
                p90_duration_ms: 0,
                p95_duration_ms: 0,
                p99_duration_ms: 0,
            },
            recent_requests: VecDeque::with_capacity(10),
            all_requests: Vec::new(),
            scroll_offset: 0,
            selected_index: 0,
            should_quit: false,
            qr_code_lines,
            ads: default_ads,
            current_ad_index: 0,
            local_open_connections: 0,
        }
    }

    /// Rotate to next ad
    pub fn rotate_ad(&mut self) {
        if !self.ads.is_empty() {
            self.current_ad_index = (self.current_ad_index + 1) % self.ads.len();
        }
    }

    /// Get current ad
    pub fn current_ad(&self) -> Option<&Ad> {
        self.ads.get(self.current_ad_index)
    }

    /// Update ads list
    pub fn set_ads(&mut self, ads: Vec<Ad>) {
        if !ads.is_empty() {
            self.ads = ads;
            self.current_ad_index = 0;
        }
    }

    /// Add a new request to the display
    pub fn add_request(&mut self, req: CapturedRequest) {
        self.all_requests.push(req.clone());
        self.recent_requests.push_back(req);
        if self.recent_requests.len() > 10 {
            self.recent_requests.pop_front();
        }
    }

    /// Update metrics
    pub fn update_metrics(&mut self, metrics: MetricsSnapshot) {
        self.metrics = metrics;
    }

    /// Update tunnel info
    pub fn update_tunnel_info(&mut self, info: TunnelInfo) {
        self.tunnel_info = info;
    }

    /// Handle key events
    pub fn handle_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            // Quit
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            // Open request list view
            (KeyCode::Char('o'), KeyModifiers::CONTROL) => {
                self.view = View::RequestList;
                self.selected_index = self.all_requests.len().saturating_sub(1);
            }
            // Back to main view
            (KeyCode::Esc, _) => {
                self.view = View::Main;
            }
            // Navigation in request list
            (KeyCode::Up | KeyCode::Char('k'), _) if matches!(self.view, View::RequestList) => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            (KeyCode::Down | KeyCode::Char('j'), _) if matches!(self.view, View::RequestList) => {
                if self.selected_index < self.all_requests.len().saturating_sub(1) {
                    self.selected_index += 1;
                }
            }
            // Page up/down
            (KeyCode::PageUp, _) if matches!(self.view, View::RequestList) => {
                self.selected_index = self.selected_index.saturating_sub(10);
            }
            (KeyCode::PageDown, _) if matches!(self.view, View::RequestList) => {
                self.selected_index = (self.selected_index + 10)
                    .min(self.all_requests.len().saturating_sub(1));
            }
            // Home/End
            (KeyCode::Home, _) if matches!(self.view, View::RequestList) => {
                self.selected_index = 0;
            }
            (KeyCode::End, _) if matches!(self.view, View::RequestList) => {
                self.selected_index = self.all_requests.len().saturating_sub(1);
            }
            _ => {}
        }
    }

    /// Handle TUI event
    pub fn handle_event(&mut self, event: TuiEvent) {
        match event {
            TuiEvent::NewRequest(req) => self.add_request(req),
            TuiEvent::MetricsUpdate(metrics) => self.update_metrics(metrics),
            TuiEvent::StatusChange(status) => self.tunnel_info.status = status,
            TuiEvent::TunnelInfoUpdate(info) => self.update_tunnel_info(info),
            TuiEvent::Key(key) => self.handle_key(key),
            TuiEvent::Tick => {} // Just triggers a redraw
            TuiEvent::AdRotate => self.rotate_ad(),
            TuiEvent::AdsUpdate(ads) => self.set_ads(ads),
            TuiEvent::ConnectionOpened => {
                self.local_open_connections += 1;
                // Update metrics display with local count if store metrics show 0
                if self.metrics.open_connections == 0 {
                    self.metrics.open_connections = self.local_open_connections;
                }
            }
            TuiEvent::ConnectionClosed => {
                self.local_open_connections = self.local_open_connections.saturating_sub(1);
                // Update metrics display with local count if store metrics show 0
                if self.metrics.open_connections == 0 || self.metrics.open_connections > self.local_open_connections {
                    self.metrics.open_connections = self.local_open_connections;
                }
            }
        }
    }
}

/// Generate QR code as text lines for terminal display
fn generate_qr_code(url: &str) -> Vec<String> {
    use qrcode::{QrCode, EcLevel};

    let code = match QrCode::with_error_correction_level(url, EcLevel::L) {
        Ok(c) => c,
        Err(_) => return vec!["[QR Error]".to_string()],
    };

    let mut lines = Vec::new();
    let modules = code.to_colors();
    let width = code.width();

    // Use half-block characters for compact display (2 rows per line)
    // ▀ = top half, ▄ = bottom half, █ = full block, ' ' = empty
    for y in (0..width).step_by(2) {
        let mut line = String::new();
        for x in 0..width {
            let top = modules[y * width + x];
            let bottom = if y + 1 < width {
                modules[(y + 1) * width + x]
            } else {
                qrcode::Color::Light
            };

            let ch = match (top, bottom) {
                (qrcode::Color::Dark, qrcode::Color::Dark) => '█',
                (qrcode::Color::Dark, qrcode::Color::Light) => '▀',
                (qrcode::Color::Light, qrcode::Color::Dark) => '▄',
                (qrcode::Color::Light, qrcode::Color::Light) => ' ',
            };
            line.push(ch);
        }
        lines.push(line);
    }

    lines
}
