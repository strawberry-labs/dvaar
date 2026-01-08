//! Request storage and broadcast for the inspector

use crate::metrics::{Metrics, MetricsSnapshot};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Maximum number of requests to store per tunnel
const MAX_REQUESTS_PER_TUNNEL: usize = 50;

/// Tunnel status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunnelStatus {
    Active,
    Disconnected,
}

/// Information about a registered tunnel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredTunnel {
    pub tunnel_id: String,
    pub subdomain: String,
    pub public_url: String,
    pub local_addr: String,
    pub status: TunnelStatus,
    pub registered_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

/// Tunnel info for the status page (legacy, kept for compatibility)
#[derive(Debug, Clone, Default, Serialize)]
pub struct TunnelInfoData {
    pub public_url: String,
    pub local_addr: String,
}

/// A captured HTTP request/response pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedRequest {
    pub id: String,
    #[serde(default)]
    pub tunnel_id: String,
    pub timestamp: DateTime<Utc>,
    pub method: String,
    pub path: String,
    pub request_headers: Vec<(String, String)>,
    #[serde(with = "base64_serde")]
    pub request_body: Vec<u8>,
    pub response_status: u16,
    pub response_headers: Vec<(String, String)>,
    #[serde(with = "base64_serde")]
    pub response_body: Vec<u8>,
    pub duration_ms: u64,
    pub size_bytes: usize,
}

/// Events broadcast to WebSocket subscribers
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum InspectorEvent {
    #[serde(rename = "request")]
    NewRequest(CapturedRequest),
    #[serde(rename = "clear")]
    Clear { tunnel_id: Option<String> },
    #[serde(rename = "tunnel_registered")]
    TunnelRegistered(RegisteredTunnel),
    #[serde(rename = "tunnel_unregistered")]
    TunnelUnregistered { tunnel_id: String },
    #[serde(rename = "tunnel_status")]
    TunnelStatusUpdate { tunnel_id: String, status: TunnelStatus },
    #[serde(rename = "tunnel_updated")]
    TunnelUpdated(RegisteredTunnel),
}

/// Store for captured requests with broadcast capability
pub struct RequestStore {
    /// Per-tunnel request storage: tunnel_id -> requests
    requests: RwLock<HashMap<String, VecDeque<CapturedRequest>>>,
    /// Registered tunnels: tunnel_id -> tunnel info
    tunnels: RwLock<HashMap<String, RegisteredTunnel>>,
    /// Per-tunnel metrics: tunnel_id -> metrics
    metrics: RwLock<HashMap<String, Arc<Metrics>>>,
    /// Broadcast channel for live updates
    broadcast_tx: broadcast::Sender<InspectorEvent>,
    /// Legacy tunnel info (for single-tunnel compatibility)
    tunnel_info: RwLock<TunnelInfoData>,
}

impl RequestStore {
    pub fn new() -> Self {
        let (broadcast_tx, _) = broadcast::channel(100);
        Self {
            requests: RwLock::new(HashMap::new()),
            tunnels: RwLock::new(HashMap::new()),
            metrics: RwLock::new(HashMap::new()),
            broadcast_tx,
            tunnel_info: RwLock::new(TunnelInfoData::default()),
        }
    }

    /// Register a new tunnel
    pub async fn register_tunnel(&self, tunnel: RegisteredTunnel) -> String {
        let tunnel_id = tunnel.tunnel_id.clone();

        // Store tunnel info
        self.tunnels.write().await.insert(tunnel_id.clone(), tunnel.clone());

        // Initialize request storage for this tunnel
        self.requests.write().await.insert(
            tunnel_id.clone(),
            VecDeque::with_capacity(MAX_REQUESTS_PER_TUNNEL),
        );

        // Initialize metrics for this tunnel
        self.metrics.write().await.insert(tunnel_id.clone(), Arc::new(Metrics::new()));

        // Update legacy tunnel info (for backwards compatibility)
        {
            let mut info = self.tunnel_info.write().await;
            info.public_url = tunnel.public_url.clone();
            info.local_addr = tunnel.local_addr.clone();
        }

        // Broadcast registration
        let _ = self.broadcast_tx.send(InspectorEvent::TunnelRegistered(tunnel));

        tunnel_id
    }

    /// Unregister a tunnel
    pub async fn unregister_tunnel(&self, tunnel_id: &str) {
        // Update status to disconnected but keep data for viewing
        if let Some(tunnel) = self.tunnels.write().await.get_mut(tunnel_id) {
            tunnel.status = TunnelStatus::Disconnected;
        }

        // Broadcast unregistration
        let _ = self.broadcast_tx.send(InspectorEvent::TunnelUnregistered {
            tunnel_id: tunnel_id.to_string(),
        });
    }

    /// Update tunnel status
    pub async fn update_tunnel_status(&self, tunnel_id: &str, status: TunnelStatus) {
        if let Some(tunnel) = self.tunnels.write().await.get_mut(tunnel_id) {
            tunnel.status = status;
            let _ = self.broadcast_tx.send(InspectorEvent::TunnelStatusUpdate {
                tunnel_id: tunnel_id.to_string(),
                status,
            });
        }
    }

    /// Update tunnel heartbeat
    pub async fn heartbeat(&self, tunnel_id: &str) {
        if let Some(tunnel) = self.tunnels.write().await.get_mut(tunnel_id) {
            tunnel.last_seen = Utc::now();
            tunnel.status = TunnelStatus::Active;
        }
    }

    /// Update tunnel's public_url (called after connection established)
    pub async fn update_tunnel_url(&self, tunnel_id: &str, public_url: String) {
        let updated_tunnel = {
            let mut tunnels = self.tunnels.write().await;
            if let Some(tunnel) = tunnels.get_mut(tunnel_id) {
                tunnel.public_url = public_url.clone();
                tunnel.last_seen = Utc::now();
                Some(tunnel.clone())
            } else {
                None
            }
        };

        // Broadcast the update so UI can refresh
        if let Some(tunnel) = updated_tunnel {
            let _ = self.broadcast_tx.send(InspectorEvent::TunnelUpdated(tunnel));
        }

        // Also update legacy tunnel info
        self.set_tunnel_info(public_url, String::new()).await;
    }

    /// Get all registered tunnels
    pub async fn get_tunnels(&self) -> Vec<RegisteredTunnel> {
        self.tunnels.read().await.values().cloned().collect()
    }

    /// Get a specific tunnel
    pub async fn get_tunnel(&self, tunnel_id: &str) -> Option<RegisteredTunnel> {
        self.tunnels.read().await.get(tunnel_id).cloned()
    }

    /// Set tunnel info for the status page (legacy method)
    pub async fn set_tunnel_info(&self, public_url: String, local_addr: String) {
        let mut info = self.tunnel_info.write().await;
        info.public_url = public_url;
        info.local_addr = local_addr;
    }

    /// Get tunnel info (legacy method)
    pub async fn get_tunnel_info(&self) -> TunnelInfoData {
        self.tunnel_info.read().await.clone()
    }

    /// Get metrics for a specific tunnel
    pub async fn get_tunnel_metrics(&self, tunnel_id: &str) -> Option<MetricsSnapshot> {
        if let Some(metrics) = self.metrics.read().await.get(tunnel_id) {
            Some(metrics.snapshot().await)
        } else {
            None
        }
    }

    /// Get shared reference to metrics for a tunnel
    #[deprecated(note = "Use metrics_for_tunnel() instead - this returns a non-persistent instance")]
    pub fn metrics(&self) -> Arc<Metrics> {
        // DEPRECATED: This returns a fresh instance every time
        // Use metrics_for_tunnel() for persistent metrics
        Arc::new(Metrics::new())
    }

    /// Get metrics for a specific tunnel
    pub async fn metrics_for_tunnel(&self, tunnel_id: &str) -> Option<Arc<Metrics>> {
        self.metrics.read().await.get(tunnel_id).cloned()
    }

    /// Get aggregated metrics snapshot across all tunnels
    pub async fn get_metrics(&self) -> MetricsSnapshot {
        // Aggregate metrics from all tunnels
        let metrics = self.metrics.read().await;
        if metrics.is_empty() {
            return MetricsSnapshot::default();
        }

        // For now, just return the first tunnel's metrics
        // TODO: properly aggregate across tunnels
        if let Some(m) = metrics.values().next() {
            m.snapshot().await
        } else {
            MetricsSnapshot::default()
        }
    }

    /// Add a captured request for a specific tunnel
    pub async fn add_request_for_tunnel(&self, tunnel_id: &str, mut request: CapturedRequest) {
        request.tunnel_id = tunnel_id.to_string();

        // Record metrics for this tunnel
        if let Some(metrics) = self.metrics.read().await.get(tunnel_id) {
            metrics.record_request(request.duration_ms).await;
        }

        let mut requests = self.requests.write().await;
        if let Some(tunnel_requests) = requests.get_mut(tunnel_id) {
            // Evict oldest if at capacity
            if tunnel_requests.len() >= MAX_REQUESTS_PER_TUNNEL {
                tunnel_requests.pop_front();
            }
            tunnel_requests.push_back(request.clone());
        }

        // Broadcast to subscribers
        let _ = self.broadcast_tx.send(InspectorEvent::NewRequest(request));
    }

    /// Add a captured request (legacy method - uses first tunnel or creates default)
    pub async fn add_request(&self, request: CapturedRequest) {
        // Get first tunnel or use empty string
        let tunnel_id = {
            let tunnels = self.tunnels.read().await;
            tunnels.keys().next().cloned().unwrap_or_default()
        };

        if tunnel_id.is_empty() {
            // No tunnel registered, store in default bucket
            let mut requests = self.requests.write().await;
            let default_requests = requests.entry(String::new()).or_insert_with(|| {
                VecDeque::with_capacity(MAX_REQUESTS_PER_TUNNEL)
            });

            if default_requests.len() >= MAX_REQUESTS_PER_TUNNEL {
                default_requests.pop_front();
            }
            default_requests.push_back(request.clone());
            let _ = self.broadcast_tx.send(InspectorEvent::NewRequest(request));
        } else {
            self.add_request_for_tunnel(&tunnel_id, request).await;
        }
    }

    /// Get requests for a specific tunnel (or all if None)
    pub async fn get_requests_for_tunnel(&self, tunnel_id: Option<&str>) -> Vec<CapturedRequest> {
        let requests = self.requests.read().await;
        match tunnel_id {
            Some(id) => requests.get(id).map(|r| r.iter().cloned().collect()).unwrap_or_default(),
            None => {
                // Return all requests from all tunnels, sorted by timestamp
                let mut all: Vec<CapturedRequest> = requests
                    .values()
                    .flat_map(|r| r.iter().cloned())
                    .collect();
                all.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
                all
            }
        }
    }

    /// Get all stored requests (legacy method)
    pub async fn get_requests(&self) -> Vec<CapturedRequest> {
        self.get_requests_for_tunnel(None).await
    }

    /// Get a specific request by ID
    pub async fn get_request(&self, id: &str) -> Option<CapturedRequest> {
        let requests = self.requests.read().await;
        for tunnel_requests in requests.values() {
            if let Some(req) = tunnel_requests.iter().find(|r| r.id == id) {
                return Some(req.clone());
            }
        }
        None
    }

    /// Clear requests for a specific tunnel (or all if None)
    pub async fn clear_tunnel(&self, tunnel_id: Option<&str>) {
        match tunnel_id {
            Some(id) => {
                if let Some(requests) = self.requests.write().await.get_mut(id) {
                    requests.clear();
                }
            }
            None => {
                for requests in self.requests.write().await.values_mut() {
                    requests.clear();
                }
            }
        }
        let _ = self.broadcast_tx.send(InspectorEvent::Clear {
            tunnel_id: tunnel_id.map(String::from),
        });
    }

    /// Clear all requests (legacy method)
    pub async fn clear(&self) {
        self.clear_tunnel(None).await;
    }

    /// Subscribe to request events
    pub fn subscribe(&self) -> broadcast::Receiver<InspectorEvent> {
        self.broadcast_tx.subscribe()
    }

    /// Cleanup stale tunnels (called periodically)
    pub async fn cleanup_stale_tunnels(&self, stale_threshold_secs: i64) {
        let now = Utc::now();
        let mut tunnels = self.tunnels.write().await;

        let stale_ids: Vec<String> = tunnels
            .iter()
            .filter(|(_, t)| {
                t.status == TunnelStatus::Active
                    && (now - t.last_seen).num_seconds() > stale_threshold_secs
            })
            .map(|(id, _)| id.clone())
            .collect();

        for id in stale_ids {
            if let Some(tunnel) = tunnels.get_mut(&id) {
                tunnel.status = TunnelStatus::Disconnected;
                let _ = self.broadcast_tx.send(InspectorEvent::TunnelStatusUpdate {
                    tunnel_id: id,
                    status: TunnelStatus::Disconnected,
                });
            }
        }
    }
}

impl Default for RequestStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Custom serialization for Vec<u8> as base64
mod base64_serde {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        STANDARD.decode(&s).map_err(serde::de::Error::custom)
    }
}
