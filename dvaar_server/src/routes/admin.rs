//! Admin routes for metrics and observability (admin.dvaar.io)

use crate::routes::AppState;
use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

/// Build the admin router
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(admin_dashboard))
        .route("/api/metrics", get(get_metrics))
        .route("/api/health", get(health_check))
        .route("/api/nodes", get(get_nodes))
}

/// Validate admin token from header or query
fn validate_admin(state: &AppState, headers: &HeaderMap) -> bool {
    let admin_token = std::env::var("ADMIN_TOKEN").unwrap_or_default();
    if admin_token.is_empty() {
        return false;
    }

    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|t| t == admin_token)
        .unwrap_or(false)
}

#[derive(Serialize)]
struct Metrics {
    timestamp: u64,
    active_tunnels: usize,
    total_users: i64,
    dau: i64,
    mau: i64,
    total_bandwidth_bytes: u64,
    node_ip: String,
    uptime_seconds: u64,
}

#[derive(Serialize)]
struct NodeInfo {
    ip: String,
    active_tunnels: usize,
    status: String,
}

static START_TIME: std::sync::OnceLock<SystemTime> = std::sync::OnceLock::new();

fn get_start_time() -> &'static SystemTime {
    START_TIME.get_or_init(SystemTime::now)
}

/// Get metrics JSON
async fn get_metrics(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !validate_admin(&state, &headers) {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    let active_tunnels = state.tunnels.len();
    let node_ip = state.config.node_ip.clone();

    // Get user stats from DB
    let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let dau: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT user_id) FROM api_keys WHERE last_used_at > NOW() - INTERVAL '24 hours'"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let mau: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT user_id) FROM api_keys WHERE last_used_at > NOW() - INTERVAL '30 days'"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    // Get total bandwidth from Redis (sum all usage keys)
    let total_bandwidth: u64 = 0; // Would need SCAN in production

    let uptime = get_start_time()
        .elapsed()
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let metrics = Metrics {
        timestamp,
        active_tunnels,
        total_users,
        dau,
        mau,
        total_bandwidth_bytes: total_bandwidth,
        node_ip,
        uptime_seconds: uptime,
    };

    Json(metrics).into_response()
}

/// Health check
async fn health_check(State(state): State<AppState>) -> Response {
    // Check DB
    let db_ok = sqlx::query("SELECT 1")
        .fetch_one(&state.db)
        .await
        .is_ok();

    // Check Redis
    let redis_ok = state.route_manager.get_route("__health_check__").await.is_ok();

    if db_ok && redis_ok {
        Json(serde_json::json!({
            "status": "healthy",
            "db": "ok",
            "redis": "ok",
            "tunnels": state.tunnels.len()
        }))
        .into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "unhealthy",
                "db": if db_ok { "ok" } else { "error" },
                "redis": if redis_ok { "ok" } else { "error" }
            })),
        )
            .into_response()
    }
}

/// Get node info (for multi-node setup)
async fn get_nodes(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !validate_admin(&state, &headers) {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    // In single-node setup, just return this node
    // In multi-node, would scan Redis for all registered nodes
    let nodes = vec![NodeInfo {
        ip: state.config.node_ip.clone(),
        active_tunnels: state.tunnels.len(),
        status: "healthy".to_string(),
    }];

    Json(nodes).into_response()
}

/// Admin dashboard HTML
async fn admin_dashboard(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !validate_admin(&state, &headers) {
        // Show login form
        return Html(ADMIN_LOGIN_HTML).into_response();
    }

    Html(ADMIN_DASHBOARD_HTML.replace("{{NODE_IP}}", &state.config.node_ip)).into_response()
}

const ADMIN_LOGIN_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Dvaar Admin</title>
    <style>
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0a0a0a; color: #fff; min-height: 100vh; display: flex; align-items: center; justify-content: center; }
        .login-box { background: #111; padding: 2rem; border-radius: 8px; width: 100%; max-width: 400px; }
        h1 { margin-bottom: 1.5rem; font-size: 1.5rem; }
        input { width: 100%; padding: 0.75rem; margin-bottom: 1rem; border: 1px solid #333; border-radius: 4px; background: #1a1a1a; color: #fff; }
        button { width: 100%; padding: 0.75rem; background: #2563eb; color: #fff; border: none; border-radius: 4px; cursor: pointer; font-weight: 600; }
        button:hover { background: #1d4ed8; }
    </style>
</head>
<body>
    <div class="login-box">
        <h1>Dvaar Admin</h1>
        <form onsubmit="login(event)">
            <input type="password" id="token" placeholder="Admin Token" required>
            <button type="submit">Login</button>
        </form>
    </div>
    <script>
        function login(e) {
            e.preventDefault();
            const token = document.getElementById('token').value;
            document.cookie = `admin_token=${token}; path=/; max-age=86400`;
            location.reload();
        }
        // Check cookie and set header
        const token = document.cookie.split('; ').find(r => r.startsWith('admin_token='))?.split('=')[1];
        if (token) {
            fetch('/api/metrics', { headers: { 'Authorization': `Bearer ${token}` } })
                .then(r => r.ok ? location.reload() : null);
        }
    </script>
</body>
</html>"#;

const ADMIN_DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Dvaar Admin</title>
    <style>
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0a0a0a; color: #fff; min-height: 100vh; }
        .container { max-width: 1200px; margin: 0 auto; padding: 2rem; }
        h1 { margin-bottom: 2rem; }
        .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; margin-bottom: 2rem; }
        .card { background: #111; padding: 1.5rem; border-radius: 8px; border: 1px solid #222; }
        .card-title { font-size: 0.875rem; color: #888; margin-bottom: 0.5rem; }
        .card-value { font-size: 2rem; font-weight: 700; }
        .card-subtitle { font-size: 0.75rem; color: #666; margin-top: 0.25rem; }
        .status { display: inline-block; width: 8px; height: 8px; border-radius: 50%; margin-right: 0.5rem; }
        .status.healthy { background: #22c55e; }
        .status.unhealthy { background: #ef4444; }
        table { width: 100%; border-collapse: collapse; background: #111; border-radius: 8px; overflow: hidden; }
        th, td { padding: 1rem; text-align: left; border-bottom: 1px solid #222; }
        th { background: #1a1a1a; font-weight: 600; }
        .refresh-btn { background: #2563eb; color: #fff; border: none; padding: 0.5rem 1rem; border-radius: 4px; cursor: pointer; margin-bottom: 1rem; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Dvaar Admin <span style="font-size: 0.875rem; color: #666;">{{NODE_IP}}</span></h1>

        <button class="refresh-btn" onclick="loadMetrics()">Refresh</button>

        <div class="grid">
            <div class="card">
                <div class="card-title">Active Tunnels</div>
                <div class="card-value" id="tunnels">-</div>
            </div>
            <div class="card">
                <div class="card-title">Total Users</div>
                <div class="card-value" id="users">-</div>
            </div>
            <div class="card">
                <div class="card-title">DAU</div>
                <div class="card-value" id="dau">-</div>
                <div class="card-subtitle">Last 24 hours</div>
            </div>
            <div class="card">
                <div class="card-title">MAU</div>
                <div class="card-value" id="mau">-</div>
                <div class="card-subtitle">Last 30 days</div>
            </div>
            <div class="card">
                <div class="card-title">Bandwidth</div>
                <div class="card-value" id="bandwidth">-</div>
                <div class="card-subtitle">Total transferred</div>
            </div>
            <div class="card">
                <div class="card-title">Uptime</div>
                <div class="card-value" id="uptime">-</div>
            </div>
        </div>

        <h2 style="margin-bottom: 1rem;">Nodes</h2>
        <table>
            <thead><tr><th>IP</th><th>Tunnels</th><th>Status</th></tr></thead>
            <tbody id="nodes"></tbody>
        </table>
    </div>
    <script>
        const token = document.cookie.split('; ').find(r => r.startsWith('admin_token='))?.split('=')[1];
        const headers = { 'Authorization': `Bearer ${token}` };

        function formatBytes(bytes) {
            if (bytes === 0) return '0 B';
            const k = 1024;
            const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
            const i = Math.floor(Math.log(bytes) / Math.log(k));
            return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
        }

        function formatUptime(seconds) {
            const d = Math.floor(seconds / 86400);
            const h = Math.floor((seconds % 86400) / 3600);
            const m = Math.floor((seconds % 3600) / 60);
            return d > 0 ? `${d}d ${h}h` : h > 0 ? `${h}h ${m}m` : `${m}m`;
        }

        async function loadMetrics() {
            try {
                const res = await fetch('/api/metrics', { headers });
                if (!res.ok) throw new Error('Unauthorized');
                const data = await res.json();
                document.getElementById('tunnels').textContent = data.active_tunnels;
                document.getElementById('users').textContent = data.total_users;
                document.getElementById('dau').textContent = data.dau;
                document.getElementById('mau').textContent = data.mau;
                document.getElementById('bandwidth').textContent = formatBytes(data.total_bandwidth_bytes);
                document.getElementById('uptime').textContent = formatUptime(data.uptime_seconds);
            } catch (e) {
                console.error(e);
            }
        }

        async function loadNodes() {
            try {
                const res = await fetch('/api/nodes', { headers });
                if (!res.ok) throw new Error('Unauthorized');
                const nodes = await res.json();
                document.getElementById('nodes').innerHTML = nodes.map(n => `
                    <tr>
                        <td>${n.ip}</td>
                        <td>${n.active_tunnels}</td>
                        <td><span class="status ${n.status}"></span>${n.status}</td>
                    </tr>
                `).join('');
            } catch (e) {
                console.error(e);
            }
        }

        loadMetrics();
        loadNodes();
        setInterval(loadMetrics, 10000);
    </script>
</body>
</html>"#;

/// Handle admin request by routing to appropriate handler
pub async fn handle_admin_request(
    State(state): State<AppState>,
    request: Request<Body>,
) -> Response {
    let path = request.uri().path();
    let headers = request.headers().clone();

    match path {
        "/" => admin_dashboard(State(state), headers).await,
        "/api/metrics" => get_metrics(State(state), headers).await,
        "/api/health" => health_check(State(state)).await,
        "/api/nodes" => get_nodes(State(state), headers).await,
        _ => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}
