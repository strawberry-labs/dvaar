//! Request storage and broadcast for the inspector

use crate::metrics::{Metrics, MetricsSnapshot};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Maximum number of requests to store
const MAX_REQUESTS: usize = 50;

/// Tunnel info for the status page
#[derive(Debug, Clone, Default, Serialize)]
pub struct TunnelInfoData {
    pub public_url: String,
    pub local_addr: String,
}

/// A captured HTTP request/response pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedRequest {
    pub id: String,
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
    Clear,
}

/// Store for captured requests with broadcast capability
pub struct RequestStore {
    requests: RwLock<VecDeque<CapturedRequest>>,
    broadcast_tx: broadcast::Sender<InspectorEvent>,
    metrics: Arc<Metrics>,
    tunnel_info: RwLock<TunnelInfoData>,
}

impl RequestStore {
    pub fn new() -> Self {
        let (broadcast_tx, _) = broadcast::channel(100);
        Self {
            requests: RwLock::new(VecDeque::with_capacity(MAX_REQUESTS)),
            broadcast_tx,
            metrics: Arc::new(Metrics::new()),
            tunnel_info: RwLock::new(TunnelInfoData::default()),
        }
    }

    /// Set tunnel info for the status page
    pub async fn set_tunnel_info(&self, public_url: String, local_addr: String) {
        let mut info = self.tunnel_info.write().await;
        info.public_url = public_url;
        info.local_addr = local_addr;
    }

    /// Get tunnel info
    pub async fn get_tunnel_info(&self) -> TunnelInfoData {
        self.tunnel_info.read().await.clone()
    }

    /// Get shared reference to metrics
    pub fn metrics(&self) -> Arc<Metrics> {
        self.metrics.clone()
    }

    /// Get current metrics snapshot
    pub async fn get_metrics(&self) -> MetricsSnapshot {
        self.metrics.snapshot().await
    }

    /// Add a captured request and broadcast to subscribers
    pub async fn add_request(&self, request: CapturedRequest) {
        // Record metrics
        self.metrics.record_request(request.duration_ms).await;

        let mut requests = self.requests.write().await;

        // Evict oldest if at capacity
        if requests.len() >= MAX_REQUESTS {
            requests.pop_front();
        }

        requests.push_back(request.clone());

        // Broadcast to subscribers (ignore if no receivers)
        let _ = self.broadcast_tx.send(InspectorEvent::NewRequest(request));
    }

    /// Get all stored requests
    pub async fn get_requests(&self) -> Vec<CapturedRequest> {
        self.requests.read().await.iter().cloned().collect()
    }

    /// Get a specific request by ID
    pub async fn get_request(&self, id: &str) -> Option<CapturedRequest> {
        self.requests
            .read()
            .await
            .iter()
            .find(|r| r.id == id)
            .cloned()
    }

    /// Clear all requests
    pub async fn clear(&self) {
        self.requests.write().await.clear();
        let _ = self.broadcast_tx.send(InspectorEvent::Clear);
    }

    /// Subscribe to request events
    pub fn subscribe(&self) -> broadcast::Receiver<InspectorEvent> {
        self.broadcast_tx.subscribe()
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
