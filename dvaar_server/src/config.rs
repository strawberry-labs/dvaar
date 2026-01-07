//! Server configuration loaded from environment variables

use std::env;
use std::net::IpAddr;

#[derive(Debug, Clone)]
pub struct Config {
    /// Host to bind to
    pub host: String,

    /// Public port for HTTP/WebSocket traffic
    pub port: u16,

    /// Internal port for node-to-node communication
    pub internal_port: u16,

    /// Base domain for main services (e.g., "dvaar.io")
    pub base_domain: String,

    /// Tunnel domain for user tunnels (e.g., "dvaar.app")
    pub tunnel_domain: String,

    /// Public URL of this server (e.g., "https://api.dvaar.io")
    pub public_url: String,

    /// This node's public IP address
    pub node_ip: String,

    /// Secret for node-to-node authentication
    pub cluster_secret: String,

    /// Allow X-Subdomain header override (local development only)
    pub allow_subdomain_header: bool,

    /// PostgreSQL connection string
    pub database_url: String,

    /// Redis connection string
    pub redis_url: String,

    /// GitHub OAuth client ID
    pub github_client_id: String,

    /// GitHub OAuth client secret
    pub github_client_secret: String,

    /// Stripe secret key (optional for MVP)
    pub stripe_secret_key: Option<String>,

    /// Stripe webhook secret (optional for MVP)
    pub stripe_webhook_secret: Option<String>,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, ConfigError> {
        let node_ip = env::var("NODE_IP").unwrap_or_else(|_| "127.0.0.1".to_string());
        let cluster_secret = env::var("CLUSTER_SECRET").unwrap_or_else(|_| "dev-cluster-secret".to_string());

        if !is_local_node(&node_ip) && cluster_secret == "dev-cluster-secret" {
            return Err(ConfigError::InsecureClusterSecret);
        }

        Ok(Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidPort)?,
            internal_port: env::var("INTERNAL_PORT")
                .unwrap_or_else(|_| "6000".to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidPort)?,
            base_domain: env::var("BASE_DOMAIN").unwrap_or_else(|_| "dvaar.io".to_string()),
            tunnel_domain: env::var("TUNNEL_DOMAIN").unwrap_or_else(|_| "dvaar.app".to_string()),
            public_url: env::var("PUBLIC_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            node_ip,
            cluster_secret,
            allow_subdomain_header: env::var("ALLOW_SUBDOMAIN_HEADER")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            database_url: env::var("DATABASE_URL")
                .map_err(|_| ConfigError::MissingEnv("DATABASE_URL"))?,
            redis_url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            github_client_id: env::var("GITHUB_CLIENT_ID")
                .unwrap_or_else(|_| String::new()),
            github_client_secret: env::var("GITHUB_CLIENT_SECRET")
                .unwrap_or_else(|_| String::new()),
            stripe_secret_key: env::var("STRIPE_SECRET_KEY").ok(),
            stripe_webhook_secret: env::var("STRIPE_WEBHOOK_SECRET").ok(),
        })
    }

    /// Get the full tunnel domain for a subdomain (e.g., "myapp.dvaar.app")
    pub fn full_domain(&self, subdomain: &str) -> String {
        format!("{}.{}", subdomain, self.tunnel_domain)
    }

    /// Get the full tunnel URL for a subdomain
    pub fn full_url(&self, subdomain: &str) -> String {
        format!("https://{}.{}", subdomain, self.tunnel_domain)
    }

    /// Get the API URL (e.g., "https://api.dvaar.io")
    pub fn api_url(&self) -> String {
        format!("https://api.{}", self.base_domain)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Missing required environment variable: {0}")]
    MissingEnv(&'static str),

    #[error("Invalid port number")]
    InvalidPort,

    #[error("CLUSTER_SECRET must be set to a secure value in non-local environments")]
    InsecureClusterSecret,
}

fn is_local_node(ip: &str) -> bool {
    if ip == "localhost" {
        return true;
    }

    ip.parse::<IpAddr>()
        .map(|addr| addr.is_loopback())
        .unwrap_or(false)
}
