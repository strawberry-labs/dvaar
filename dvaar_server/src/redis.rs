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

    /// Increment bandwidth usage for a user
    pub async fn increment_usage(&self, user_id: &str, bytes: u64) -> anyhow::Result<u64> {
        let key = format!("{}{}", constants::USAGE_PREFIX, user_id);
        let result: i64 = self.client.incr_by(&key, bytes as i64).await?;
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
