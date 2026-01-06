//! Route handlers for the Dvaar server

pub mod admin;
pub mod auth;
pub mod billing;
pub mod ingress;
pub mod proxy;
pub mod tunnel;

use crate::{abuse::RateLimiter, config::Config, redis::RouteManager};
use dashmap::DashMap;
use fred::clients::Client as RedisClient;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: PgPool,
    pub redis: RedisClient,
    pub route_manager: Arc<RouteManager>,
    pub rate_limiter: RateLimiter,
    /// Local tunnel connections: subdomain -> tunnel sender
    pub tunnels: Arc<DashMap<String, TunnelHandle>>,
    /// Shared HTTP client for inter-node communication (connection pooling)
    pub http_client: reqwest::Client,
}

/// Handle to a tunnel connection
#[derive(Debug)]
pub struct TunnelHandle {
    /// Channel to send HTTP requests to the tunnel
    pub request_tx: mpsc::Sender<TunnelRequest>,
    /// User ID that owns this tunnel
    pub user_id: String,
}

/// A request to be sent through the tunnel
#[derive(Debug)]
pub struct TunnelRequest {
    /// The HTTP request packet
    pub request: dvaar_common::HttpRequestPacket,
    /// Channel to receive the response
    pub response_tx: tokio::sync::oneshot::Sender<TunnelResponse>,
}

/// Response from the tunnel
#[derive(Debug)]
pub enum TunnelResponse {
    Success(dvaar_common::HttpResponsePacket),
    Error(String),
    Timeout,
}

impl AppState {
    pub async fn new(config: Config, db: PgPool, redis: RedisClient) -> Self {
        let route_manager = Arc::new(RouteManager::new(redis.clone()));
        let rate_limiter = RateLimiter::new(Arc::new(redis.clone()));

        // Create shared HTTP client with connection pooling
        let http_client = reqwest::Client::builder()
            .pool_max_idle_per_host(100)
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config: Arc::new(config),
            db,
            redis,
            route_manager,
            rate_limiter,
            tunnels: Arc::new(DashMap::new()),
            http_client,
        }
    }
}
