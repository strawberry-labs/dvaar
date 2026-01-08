//! Request storage and broadcast for the inspector

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tokio::sync::{broadcast, RwLock};

/// Maximum number of requests to store
const MAX_REQUESTS: usize = 50;

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
}

impl RequestStore {
    pub fn new() -> Self {
        let (broadcast_tx, _) = broadcast::channel(100);
        Self {
            requests: RwLock::new(VecDeque::with_capacity(MAX_REQUESTS)),
            broadcast_tx,
        }
    }

    /// Add a captured request and broadcast to subscribers
    pub async fn add_request(&self, request: CapturedRequest) {
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
