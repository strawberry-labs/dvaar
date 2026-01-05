//! Dvaar Server - Distributed tunneling edge node
//!
//! This server handles:
//! - WebSocket tunnel connections from CLI clients
//! - Public HTTP ingress to tunneled services
//! - Node-to-node proxy for distributed routing
//! - GitHub OAuth authentication
//! - Bandwidth metering

mod abuse;
mod config;
mod db;
mod redis;
mod routes;
mod services;

use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use axum_extra::extract::Host;
use std::net::SocketAddr;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,dvaar_server=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = config::Config::from_env()?;
    tracing::info!("Starting Dvaar server on {}:{}", config.host, config.port);
    tracing::info!("Base domain: {} (API, admin, docs)", config.base_domain);
    tracing::info!("Tunnel domain: {} (user tunnels)", config.tunnel_domain);
    tracing::info!("Node IP: {}", config.node_ip);

    // Initialize database
    tracing::info!("Connecting to database...");
    let db_pool = db::init_pool(&config.database_url).await?;
    tracing::info!("Running database migrations...");
    db::run_migrations(&db_pool).await?;

    // Initialize Redis
    tracing::info!("Connecting to Redis...");
    let redis_client = redis::init_client(&config.redis_url).await?;

    // Create app state
    let state = routes::AppState::new(config.clone(), db_pool, redis_client).await;

    // Build main router (public port)
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/health", get(health_check))
        .route("/_caddy/check", get(caddy_check))
        .merge(routes::auth::router())
        .merge(routes::billing::router())
        .merge(routes::tunnel::router())
        .fallback(handle_fallback)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state.clone());

    // Build internal router (for node-to-node communication)
    let internal_app = Router::new()
        .merge(routes::proxy::router())
        .with_state(state.clone());

    // Start servers
    let public_addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    let internal_addr: SocketAddr = format!("{}:{}", config.host, config.internal_port).parse()?;

    tracing::info!("Public server listening on {}", public_addr);
    tracing::info!("Internal server listening on {}", internal_addr);

    // Run both servers
    let public_server = async {
        let listener = tokio::net::TcpListener::bind(public_addr).await?;
        axum::serve(listener, app).await
    };

    let internal_server = async {
        let listener = tokio::net::TcpListener::bind(internal_addr).await?;
        axum::serve(listener, internal_app).await
    };

    tokio::select! {
        result = public_server => {
            if let Err(e) = result {
                tracing::error!("Public server error: {}", e);
            }
        }
        result = internal_server => {
            if let Err(e) = result {
                tracing::error!("Internal server error: {}", e);
            }
        }
    }

    Ok(())
}

/// Caddy on-demand TLS check - validates subdomain before cert provisioning
async fn caddy_check(
    State(state): State<routes::AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let domain = match params.get("domain") {
        Some(d) => d,
        None => return StatusCode::BAD_REQUEST,
    };

    // Extract subdomain from domain (e.g., "foo.dvaar.app" -> "foo")
    let subdomain = domain
        .strip_suffix(&format!(".{}", state.config.tunnel_domain))
        .unwrap_or(domain);

    // Check if tunnel exists for this subdomain
    if state.tunnels.contains_key(subdomain) {
        StatusCode::OK
    } else {
        // Also check Redis for tunnels on other nodes
        match state.route_manager.get_route(subdomain).await {
            Ok(Some(_)) => StatusCode::OK,
            _ => StatusCode::NOT_FOUND,
        }
    }
}

/// Health check endpoint
async fn health_check(State(state): State<routes::AppState>) -> impl IntoResponse {
    let db_status = sqlx::query("SELECT 1")
        .fetch_one(&state.db)
        .await
        .map(|_| "ok")
        .unwrap_or("error");

    let redis_status = state
        .route_manager
        .ping()
        .await
        .map(|_| "ok")
        .unwrap_or("error");

    let tunnels = state.tunnels.len();

    let status = if db_status == "ok" && redis_status == "ok" {
        "healthy"
    } else {
        "degraded"
    };

    axum::Json(serde_json::json!({
        "status": status,
        "db": db_status,
        "redis": redis_status,
        "tunnels": tunnels
    }))
}

/// Fallback handler - routes to admin or ingress based on domain
async fn handle_fallback(
    State(state): State<routes::AppState>,
    Host(host): Host,
    request: Request<Body>,
) -> impl IntoResponse {
    let host_str = host.split(':').next().unwrap_or(&host);
    let base_domain = &state.config.base_domain;

    // Check if this is admin.dvaar.io or dash.dvaar.io (base domain subdomains)
    if host_str == format!("admin.{}", base_domain)
        || host_str == format!("dash.{}", base_domain)
        || host_str == "admin.localhost"
    {
        return routes::admin::handle_admin_request(State(state.clone()), request).await;
    }

    // Everything else (*.dvaar.app or custom domains) goes to ingress
    routes::ingress::handle_ingress(State(state), Host(host), request).await
}
