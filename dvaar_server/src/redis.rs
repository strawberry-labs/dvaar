//! Redis connection and operations for routing

use dashmap::DashMap;
use dvaar_common::{constants, RouteInfo};
use fred::clients::Client;
use fred::interfaces::*;
use fred::types::{config::Config as RedisConfig, Expiration};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Initialize Redis client
pub async fn init_client(redis_url: &str) -> anyhow::Result<Client> {
    let config = RedisConfig::from_url(redis_url)?;
    let client = Client::new(config, None, None, None);
    client.init().await?;
    Ok(client)
}

/// Local cache entry with timestamp
struct CacheEntry {
    route: RouteInfo,
    cached_at: Instant,
}

/// Cache TTL - routes are cached locally for 5 seconds
const ROUTE_CACHE_TTL: Duration = Duration::from_secs(5);

/// Redis operations for route management
pub struct RouteManager {
    client: Client,
    /// Local cache for route lookups to reduce Redis hits on hot path
    route_cache: Arc<DashMap<String, CacheEntry>>,
}

impl RouteManager {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            route_cache: Arc::new(DashMap::new()),
        }
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

        // Update local cache
        self.route_cache.insert(
            subdomain.to_string(),
            CacheEntry {
                route: route_info.clone(),
                cached_at: Instant::now(),
            },
        );

        Ok(())
    }

    /// Get route info for a subdomain (with local caching)
    pub async fn get_route(&self, subdomain: &str) -> anyhow::Result<Option<RouteInfo>> {
        // Check local cache first
        if let Some(entry) = self.route_cache.get(subdomain) {
            if entry.cached_at.elapsed() < ROUTE_CACHE_TTL {
                return Ok(Some(entry.route.clone()));
            }
            // Cache entry expired, remove it
            drop(entry);
            self.route_cache.remove(subdomain);
        }

        // Fetch from Redis
        let key = format!("{}{}", constants::ROUTE_PREFIX, subdomain);
        let value: Option<String> = self.client.get(&key).await?;

        match value {
            Some(json) => {
                let route = RouteInfo::from_json(&json)?;
                // Cache the result
                self.route_cache.insert(
                    subdomain.to_string(),
                    CacheEntry {
                        route: route.clone(),
                        cached_at: Instant::now(),
                    },
                );
                Ok(Some(route))
            }
            None => Ok(None),
        }
    }

    /// Remove a route (on disconnect)
    pub async fn remove_route(&self, subdomain: &str) -> anyhow::Result<()> {
        let key = format!("{}{}", constants::ROUTE_PREFIX, subdomain);
        self.client.del::<i64, _>(&key).await?;
        // Invalidate local cache
        self.route_cache.remove(subdomain);
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

    /// Increment bandwidth usage for a user with a desired TTL (seconds)
    pub async fn increment_usage(&self, user_id: &str, bytes: u64, ttl_secs: i64) -> anyhow::Result<u64> {
        let key = format!("{}{}", constants::USAGE_PREFIX, user_id);
        let result: i64 = self.client.incr_by(&key, bytes as i64).await?;

        if ttl_secs > 0 {
            let ttl: i64 = self.client.ttl(&key).await.unwrap_or(-2);
            let drift_allowance_secs: i64 = 60;

            if ttl < 0 || ttl > ttl_secs + drift_allowance_secs || ttl_secs - ttl > drift_allowance_secs {
                self.client.expire::<(), _>(&key, ttl_secs, None).await?;
            }
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

    /// Store ads configuration (for CLI to fetch)
    pub async fn store_ads(&self, key: &str, json_value: &str) -> anyhow::Result<()> {
        self.client
            .set::<(), _, _>(
                key,
                json_value,
                None, // No expiration for ads
                None,
                false,
            )
            .await?;
        Ok(())
    }

    /// Get ads configuration
    pub async fn get_ads(&self, key: &str) -> anyhow::Result<Option<String>> {
        let value: Option<String> = self.client.get(key).await?;
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

    /// Atomically check and increment concurrent tunnel count for a user.
    /// Returns (new_count, was_allowed) - if was_allowed is false, the increment was rolled back.
    /// This is atomic: INCR first, then check limit, DECR if over.
    pub async fn try_increment_user_tunnels(&self, user_id: &str, limit: u32) -> anyhow::Result<(u32, bool)> {
        let key = format!("{}{}", constants::USER_TUNNELS_PREFIX, user_id);

        // Atomically increment
        let new_count: i64 = self.client.incr(&key).await?;

        // Check if we exceeded the limit (count was already at or above limit before increment)
        if new_count > limit as i64 {
            // Roll back the increment
            let _: i64 = self.client.decr(&key).await?;
            return Ok((new_count.saturating_sub(1) as u32, false));
        }

        // Set short TTL - heartbeat (30s) keeps it alive
        // If tunnel dies without cleanup, count auto-expires in ~2 min
        self.client.expire::<(), _>(&key, constants::USER_TUNNELS_TTL_SECONDS, None).await?;

        Ok((new_count as u32, true))
    }

    /// Decrement concurrent tunnel count for a user
    pub async fn decrement_user_tunnels(&self, user_id: &str) -> anyhow::Result<()> {
        let key = format!("{}{}", constants::USER_TUNNELS_PREFIX, user_id);
        let result: i64 = self.client.decr(&key).await?;
        // Clean up if count drops to zero or below
        if result <= 0 {
            let _ = self.client.del::<(), _>(&key).await;
        }
        Ok(())
    }

    /// Refresh TTL on user tunnel count (called during heartbeat)
    pub async fn refresh_user_tunnels_ttl(&self, user_id: &str) -> anyhow::Result<()> {
        let key = format!("{}{}", constants::USER_TUNNELS_PREFIX, user_id);
        // Refresh TTL - heartbeat runs every 30s so this keeps it alive
        self.client.expire::<(), _>(&key, constants::USER_TUNNELS_TTL_SECONDS, None).await?;
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

/// Start a heartbeat task that refreshes a route and user tunnel count periodically
pub fn spawn_heartbeat(
    route_manager: RouteManager,
    subdomain: String,
    user_id: String,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let interval = Duration::from_secs(constants::HEARTBEAT_INTERVAL_SECONDS);

        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {
                    // Refresh route TTL
                    if let Err(e) = route_manager.refresh_route(&subdomain).await {
                        tracing::error!("Failed to refresh route for {}: {}", subdomain, e);
                    } else {
                        tracing::debug!("Refreshed route for {}", subdomain);
                    }

                    // Refresh user tunnel count TTL
                    if let Err(e) = route_manager.refresh_user_tunnels_ttl(&user_id).await {
                        tracing::error!("Failed to refresh user tunnel TTL for {}: {}", user_id, e);
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
