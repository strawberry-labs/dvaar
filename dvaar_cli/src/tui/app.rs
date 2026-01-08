//! TUI application state and event handling

use crate::inspector::CapturedRequest;
use crate::metrics::MetricsSnapshot;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::VecDeque;

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
}

impl TuiApp {
    pub fn new(tunnel_info: TunnelInfo) -> Self {
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
        }
    }
}
