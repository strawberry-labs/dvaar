//! Port detection and binding logic for the inspector

use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tokio::net::TcpListener;

/// Maximum number of ports to try before giving up
const MAX_PORT_ATTEMPTS: u16 = 10;

/// How the inspector should operate
#[derive(Debug, Clone)]
pub enum InspectorMode {
    /// Start a new inspector server on this port
    Server(u16),
    /// Connect to existing inspector as client on this port
    Client(u16),
}

/// Health check response from inspector
#[derive(Debug, Deserialize)]
struct HealthResponse {
    service: String,
}

/// Result of checking a single port
enum PortCheckResult {
    /// Port is available for binding
    Available,
    /// Port has a dvaar inspector running
    DvaarInspector,
    /// Port is used by something else
    UsedByOther,
}

/// Check if a port has a dvaar inspector running
async fn check_port_for_dvaar(port: u16) -> PortCheckResult {
    let client = match Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
    {
        Ok(c) => c,
        Err(_) => return PortCheckResult::UsedByOther,
    };

    let health_url = format!("http://127.0.0.1:{}/api/health", port);

    match client.get(&health_url).send().await {
        Ok(response) if response.status().is_success() => {
            // Something is responding, check if it's dvaar
            match response.json::<HealthResponse>().await {
                Ok(health) if health.service == "dvaar-inspector" => {
                    PortCheckResult::DvaarInspector
                }
                _ => PortCheckResult::UsedByOther,
            }
        }
        Ok(_) => PortCheckResult::UsedByOther,
        Err(e) => {
            // Connection refused likely means port is free
            if e.is_connect() {
                PortCheckResult::Available
            } else {
                PortCheckResult::UsedByOther
            }
        }
    }
}

/// Try to bind to a port to check if it's truly available
async fn can_bind_port(port: u16) -> bool {
    TcpListener::bind(format!("127.0.0.1:{}", port)).await.is_ok()
}

/// Find an available port for the inspector
///
/// This function tries the preferred port first. If it's in use:
/// - If it's a dvaar inspector, return Client mode to connect to it
/// - If it's something else, try the next port
///
/// Continues trying ports until finding one that's:
/// - Available for binding (Server mode)
/// - Has a dvaar inspector (Client mode)
pub async fn find_inspector_port(preferred_port: u16) -> Result<InspectorMode> {
    for offset in 0..MAX_PORT_ATTEMPTS {
        let port = preferred_port + offset;

        // First, check if there's already a dvaar inspector on this port
        match check_port_for_dvaar(port).await {
            PortCheckResult::DvaarInspector => {
                tracing::debug!("Found existing dvaar inspector on port {}", port);
                return Ok(InspectorMode::Client(port));
            }
            PortCheckResult::Available => {
                // Double-check by actually trying to bind
                if can_bind_port(port).await {
                    tracing::debug!("Port {} is available for inspector", port);
                    return Ok(InspectorMode::Server(port));
                }
                // Couldn't bind, something else grabbed it, try next
                tracing::debug!("Port {} appeared available but couldn't bind, trying next", port);
            }
            PortCheckResult::UsedByOther => {
                tracing::debug!("Port {} is used by non-dvaar service, trying next", port);
            }
        }
    }

    anyhow::bail!(
        "Could not find available port for inspector (tried {} ports starting from {})",
        MAX_PORT_ATTEMPTS,
        preferred_port
    )
}

/// Get the actual port from an InspectorMode
impl InspectorMode {
    pub fn port(&self) -> u16 {
        match self {
            InspectorMode::Server(p) | InspectorMode::Client(p) => *p,
        }
    }

    pub fn is_server(&self) -> bool {
        matches!(self, InspectorMode::Server(_))
    }

    pub fn is_client(&self) -> bool {
        matches!(self, InspectorMode::Client(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_find_available_port() {
        // Should find an available port starting from a high number
        let result = find_inspector_port(49000).await;
        assert!(result.is_ok());
        let mode = result.unwrap();
        assert!(mode.is_server());
        assert!(mode.port() >= 49000);
    }
}
