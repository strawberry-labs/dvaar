//! Inspector client for connecting to an existing inspector server
//!
//! When a tunnel connects to an existing inspector (instead of starting its own),
//! it uses this client to register itself and submit requests.

use super::store::CapturedRequest;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Client for communicating with an existing inspector server
#[derive(Clone)]
pub struct InspectorClient {
    base_url: String,
    tunnel_id: String,
    client: Client,
    registered: Arc<RwLock<bool>>,
}

/// Request to register a tunnel
#[derive(Debug, Serialize)]
struct RegisterTunnelRequest {
    tunnel_id: String,
    subdomain: String,
    public_url: String,
    local_addr: String,
}

/// Response from tunnel registration
#[derive(Debug, Deserialize)]
struct RegisterTunnelResponse {
    success: bool,
    #[allow(dead_code)]
    tunnel_id: String,
}

impl InspectorClient {
    /// Create a new inspector client
    pub fn new(port: u16, tunnel_id: String) -> Self {
        Self {
            base_url: format!("http://127.0.0.1:{}", port),
            tunnel_id,
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("Failed to build HTTP client"),
            registered: Arc::new(RwLock::new(false)),
        }
    }

    /// Get the tunnel ID
    pub fn tunnel_id(&self) -> &str {
        &self.tunnel_id
    }

    /// Get the inspector URL
    pub fn inspector_url(&self) -> &str {
        &self.base_url
    }

    /// Register this tunnel with the inspector
    pub async fn register(
        &self,
        subdomain: &str,
        public_url: &str,
        local_addr: &str,
    ) -> Result<()> {
        let url = format!("{}/api/tunnels/register", self.base_url);

        let request = RegisterTunnelRequest {
            tunnel_id: self.tunnel_id.clone(),
            subdomain: subdomain.to_string(),
            public_url: public_url.to_string(),
            local_addr: local_addr.to_string(),
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to register with inspector")?;

        if !response.status().is_success() {
            anyhow::bail!("Inspector registration failed: {}", response.status());
        }

        let result: RegisterTunnelResponse = response
            .json()
            .await
            .context("Failed to parse registration response")?;

        if !result.success {
            anyhow::bail!("Inspector registration was not successful");
        }

        *self.registered.write().await = true;
        tracing::debug!("Registered tunnel {} with inspector", self.tunnel_id);

        Ok(())
    }

    /// Unregister this tunnel from the inspector
    pub async fn unregister(&self) -> Result<()> {
        if !*self.registered.read().await {
            return Ok(());
        }

        let url = format!(
            "{}/api/tunnels/{}/unregister",
            self.base_url, self.tunnel_id
        );

        // Best effort - don't fail if unregister fails
        match self.client.post(&url).send().await {
            Ok(response) if response.status().is_success() => {
                tracing::debug!("Unregistered tunnel {} from inspector", self.tunnel_id);
            }
            Ok(response) => {
                tracing::warn!(
                    "Failed to unregister tunnel {}: {}",
                    self.tunnel_id,
                    response.status()
                );
            }
            Err(e) => {
                tracing::warn!("Failed to unregister tunnel {}: {}", self.tunnel_id, e);
            }
        }

        *self.registered.write().await = false;
        Ok(())
    }

    /// Send a captured request to the inspector
    pub async fn submit_request(&self, request: CapturedRequest) -> Result<()> {
        let url = format!(
            "{}/api/tunnels/{}/request",
            self.base_url, self.tunnel_id
        );

        self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to submit request to inspector")?;

        Ok(())
    }

    /// Send heartbeat to keep tunnel alive
    pub async fn heartbeat(&self) -> Result<()> {
        let url = format!(
            "{}/api/tunnels/{}/heartbeat",
            self.base_url, self.tunnel_id
        );

        self.client
            .post(&url)
            .send()
            .await
            .context("Failed to send heartbeat")?;

        Ok(())
    }

    /// Check if the inspector is still running
    pub async fn is_inspector_alive(&self) -> bool {
        let url = format!("{}/api/health", self.base_url);
        self.client.get(&url).send().await.is_ok()
    }

    /// Start a background heartbeat task
    pub fn start_heartbeat_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                if self.heartbeat().await.is_err() {
                    tracing::warn!("Heartbeat to inspector failed");
                    // Don't break - the inspector might come back
                }
            }
        })
    }
}

impl Drop for InspectorClient {
    fn drop(&mut self) {
        // Best-effort unregister on drop
        // We can't await in drop, so spawn a task
        let url = format!(
            "{}/api/tunnels/{}/unregister",
            self.base_url, self.tunnel_id
        );
        let client = self.client.clone();

        tokio::spawn(async move {
            let _ = client.post(&url).send().await;
        });
    }
}
