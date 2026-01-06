//! Redis connection and operations for routing

use dvaar_common::{constants, RouteInfo};
use fred::clients::Client;
use fred::interfaces::*;
use fred::types::{config::Config as RedisConfig, Expiration};
use std::time::Duration;

/// Initialize Redis client
pub async fn init_client(redis_url: &str) -> anyhow::Result<Client> {
    let config = RedisConfig::from_url(redis_url)?;
    let client = Client::new(config, None, None, None);
    client.init().await?;
    Ok(client)
}

/// Redis operations for route management
pub struct RouteManager {
    client: Client,
}

impl RouteManager {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Ping Redis to check connection
    pub async fn ping(&self) -> anyhow::Result<()> {
        self.client.ping::<()>(None).await?;
        Ok(())
    }

    /// Register a route for a subdomain
    pub async fn register_route(
        &self,
        subdomain: &str,
        route_info: &RouteInfo,
    ) -> anyhow::Result<()> {
        let key = format!("{}{}", constants::ROUTE_PREFIX, subdomain);
        let value = route_info.to_json()?;

        self.client
            .set::<(), _, _>(
                &key,
                value,
                Some(Expiration::EX(constants::ROUTE_TTL_SECONDS as i64)),
                None,
                false,
            )
            .await?;

        Ok(())
    }

    /// Get route info for a subdomain
    pub async fn get_route(&self, subdomain: &str) -> anyhow::Result<Option<RouteInfo>> {
        let key = format!("{}{}", constants::ROUTE_PREFIX, subdomain);
        let value: Option<String> = self.client.get(&key).await?;

        match value {
            Some(json) => {
                let route = RouteInfo::from_json(&json)?;
                Ok(Some(route))
            }
            None => Ok(None),
        }
    }

    /// Remove a route (on disconnect)
    pub async fn remove_route(&self, subdomain: &str) -> anyhow::Result<()> {
        let key = format!("{}{}", constants::ROUTE_PREFIX, subdomain);
        self.client.del::<i64, _>(&key).await?;
        Ok(())
    }

    /// Refresh route TTL (heartbeat)
    pub async fn refresh_route(&self, subdomain: &str) -> anyhow::Result<bool> {
        let key = format!("{}{}", constants::ROUTE_PREFIX, subdomain);
        let result: bool = self
            .client
            .expire(&key, constants::ROUTE_TTL_SECONDS as i64, None)
            .await?;
        Ok(result)
    }

    /// Increment bandwidth usage for a user (with monthly TTL for auto-reset)
    pub async fn increment_usage(&self, user_id: &str, bytes: u64) -> anyhow::Result<u64> {
        let key = format!("{}{}", constants::USAGE_PREFIX, user_id);
        let result: i64 = self.client.incr_by(&key, bytes as i64).await?;

        // Set TTL to end of current month (30 days from first usage as approximation)
        // This ensures usage resets monthly even without explicit reset job
        let ttl: i64 = self.client.ttl(&key).await?;
        if ttl < 0 {
            // No TTL set yet, set 30 day TTL
            self.client.expire::<(), _>(&key, 30 * 24 * 60 * 60, None).await?;
        }

        Ok(result as u64)
    }

    /// Get bandwidth usage for a user
    pub async fn get_usage(&self, user_id: &str) -> anyhow::Result<u64> {
        let key = format!("{}{}", constants::USAGE_PREFIX, user_id);
        let value: Option<i64> = self.client.get(&key).await?;
        Ok(value.unwrap_or(0) as u64)
    }

    /// Reset bandwidth usage (e.g., monthly reset)
    pub async fn reset_usage(&self, user_id: &str) -> anyhow::Result<()> {
        let key = format!("{}{}", constants::USAGE_PREFIX, user_id);
        self.client.del::<i64, _>(&key).await?;
        Ok(())
    }

    /// Store OAuth state with TTL (for CSRF protection)
    pub async fn store_oauth_state(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.client
            .set::<(), _, _>(
                key,
                value,
                Some(Expiration::EX(600)), // 10 minute TTL
                None,
                false,
            )
            .await?;
        Ok(())
    }

    /// Get and delete OAuth state atomically (prevents replay attacks)
    pub async fn get_and_delete_oauth_state(&self, key: &str) -> anyhow::Result<Option<String>> {
        let value: Option<String> = self.client.getdel(key).await?;
        Ok(value)
    }

    /// Register this node in the cluster (uses individual keys with TTL per node)
    pub async fn register_node(&self, node_id: &str, node_info: &NodeInfo) -> anyhow::Result<()> {
        let key = format!("{}:{}", constants::NODE_PREFIX, node_id);
        let value = serde_json::to_string(node_info)?;

        // Each node has its own key with individual TTL - stale nodes expire independently
        self.client
            .set::<(), _, _>(
                &key,
                value,
                Some(Expiration::EX(constants::NODE_TTL_SECONDS as i64)),
                None,
                false,
            )
            .await?;

        // Also add to a set for easy enumeration (set members don't have TTL issues)
        self.client.sadd::<(), _, _>("nodes:active", node_id).await?;

        Ok(())
    }

    /// Update node's tunnel count
    pub async fn update_node_tunnels(&self, node_id: &str, tunnel_count: u32) -> anyhow::Result<()> {
        let key = format!("{}:{}", constants::NODE_PREFIX, node_id);

        if let Some(json) = self.client.get::<Option<String>, _>(&key).await? {
            if let Ok(mut info) = serde_json::from_str::<NodeInfo>(&json) {
                info.tunnel_count = tunnel_count;
                let value = serde_json::to_string(&info)?;
                // Refresh TTL on update
                self.client
                    .set::<(), _, _>(
                        &key,
                        value,
                        Some(Expiration::EX(constants::NODE_TTL_SECONDS as i64)),
                        None,
                        false,
                    )
                    .await?;
            }
        }
        Ok(())
    }

    /// Get all registered nodes
    pub async fn get_all_nodes(&self) -> anyhow::Result<Vec<NodeInfo>> {
        // Get all node IDs from the set
        let node_ids: Vec<String> = self.client.smembers("nodes:active").await?;

        let mut nodes = Vec::new();
        let mut stale_nodes = Vec::new();

        for node_id in node_ids {
            let key = format!("{}:{}", constants::NODE_PREFIX, node_id);
            if let Some(json) = self.client.get::<Option<String>, _>(&key).await? {
                if let Ok(info) = serde_json::from_str::<NodeInfo>(&json) {
                    nodes.push(info);
                }
            } else {
                // Node key expired but still in set - mark for cleanup
                stale_nodes.push(node_id);
            }
        }

        // Clean up stale nodes from the set
        for stale_id in stale_nodes {
            let _ = self.client.srem::<(), _, _>("nodes:active", &stale_id).await;
        }

        Ok(nodes)
    }

    /// Unregister this node from the cluster
    pub async fn unregister_node(&self, node_id: &str) -> anyhow::Result<()> {
        let key = format!("{}:{}", constants::NODE_PREFIX, node_id);
        self.client.del::<(), _>(&key).await?;
        self.client.srem::<(), _, _>("nodes:active", node_id).await?;
        Ok(())
    }
}

/// Node information for cluster discovery
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeInfo {
    pub node_id: String,
    pub ip: String,
    pub port: u16,
    pub region: Option<String>,
    pub tunnel_count: u32,
    pub max_tunnels: u32,
}

/// Start a heartbeat task that refreshes a route periodically
pub fn spawn_heartbeat(
    route_manager: RouteManager,
    subdomain: String,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let interval = Duration::from_secs(constants::HEARTBEAT_INTERVAL_SECONDS);

        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {
                    if let Err(e) = route_manager.refresh_route(&subdomain).await {
                        tracing::error!("Failed to refresh route for {}: {}", subdomain, e);
                    } else {
                        tracing::debug!("Refreshed route for {}", subdomain);
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("Heartbeat task for {} shutting down", subdomain);
                        break;
                    }
                }
            }
        }
    })
}
