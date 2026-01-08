//! HTTP tunnel command

use crate::config::{generate_session_id, logs_dir, Config, Session, Sessions};
use crate::inspector::{find_inspector_port, InspectorClient, InspectorMode, RegisteredTunnel, RequestStore, TunnelStatus};
use crate::tunnel::client::TunnelClient;
use anyhow::{Context, Result};
use chrono::Utc;
use console::style;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use uuid::Uuid;

/// HTTP tunnel options
#[derive(Debug, Clone)]
pub struct HttpOptions {
    pub target: String,
    pub subdomain: Option<String>,
    pub auth: Option<String>,
    pub host_header: Option<String>,
    pub detach: bool,
    pub use_tls: bool,
    pub inspect_port: Option<u16>,
    pub tui_mode: bool,
}

/// Handle HTTP tunnel command
pub async fn run(opts: HttpOptions) -> Result<()> {
    let config = Config::load()?;
    let token = config.require_auth()?;

    // Parse target
    let (target_addr, static_dir) = parse_target(&opts.target)?;

    // If detaching, spawn background process
    if opts.detach {
        return spawn_background(opts).await;
    }

    // Start static file server if needed
    let _static_server = if let Some(ref dir) = static_dir {
        let server = start_static_server(dir.clone()).await?;
        Some(server)
    } else {
        None
    };

    // Use static server address if we started one
    let actual_target = if let Some(ref server) = _static_server {
        format!("localhost:{}", server.addr.port())
    } else {
        target_addr
    };

    // Generate unique tunnel ID
    let tunnel_id = Uuid::new_v4().to_string();

    // Determine inspector mode if inspector is enabled
    let (inspector_store, inspector_client, actual_inspect_port, _inspector_handle) =
        if let Some(port) = opts.inspect_port {
            match find_inspector_port(port).await? {
                InspectorMode::Server(actual_port) => {
                    // We're the first tunnel - start the inspector server
                    let store = Arc::new(RequestStore::new());
                    let handle = crate::inspector::start_server(actual_port, store.clone()).await?;

                    // Register ourselves as the primary tunnel
                    // (public_url will be set after connection)
                    store.register_tunnel(RegisteredTunnel {
                        tunnel_id: tunnel_id.clone(),
                        subdomain: opts.subdomain.clone().unwrap_or_default(),
                        public_url: String::new(),
                        local_addr: actual_target.clone(),
                        status: TunnelStatus::Active,
                        registered_at: Utc::now(),
                        last_seen: Utc::now(),
                    }).await;

                    (Some(store), None, Some(actual_port), Some(handle))
                }
                InspectorMode::Client(actual_port) => {
                    // Inspector already running - connect as client
                    let client = InspectorClient::new(actual_port, tunnel_id.clone());
                    (None, Some(client), Some(actual_port), None)
                }
            }
        } else {
            (None, None, None, None)
        };

    let mut client = TunnelClient::new(
        &config.websocket_url(),
        token,
        opts.subdomain.clone(),
        actual_target.clone(),
    );

    // Set user info from config
    client.set_user_info(config.user_email.clone(), config.user_plan.clone());

    // Handle basic auth if provided
    if let Some(auth) = &opts.auth {
        client.set_basic_auth(auth);
    }

    // Handle host header override
    if let Some(host) = &opts.host_header {
        client.set_host_header(host);
    }

    // Set TLS mode
    client.set_upstream_tls(opts.use_tls);

    // Set inspector store or client
    if let Some(store) = inspector_store {
        client.set_inspector(store);
    }
    if let Some(inspector_client) = inspector_client {
        client.set_inspector_client(inspector_client);
    }

    // Set tunnel ID for registration
    client.set_tunnel_id(tunnel_id);

    // Run the tunnel
    let result = client.run(actual_inspect_port, opts.tui_mode).await;

    if let Err(e) = result {
        if opts.tui_mode {
            // TUI will have restored terminal, just print error
            eprintln!("Tunnel error: {}", e);
        } else {
            cliclack::outro_cancel(format!("Tunnel error: {}", e))?;
        }
    }

    Ok(())
}

/// Parse the target argument
fn parse_target(target: &str) -> Result<(String, Option<PathBuf>)> {
    // Check if it's a path (static file serving)
    let path = PathBuf::from(target);
    if path.exists() && path.is_dir() {
        // Start static file server
        // Use a random port
        let port = 9000 + rand::random::<u16>() % 1000;
        return Ok((format!("localhost:{}", port), Some(path)));
    }

    // Check if it's just a port number
    if let Ok(port) = target.parse::<u16>() {
        return Ok((format!("localhost:{}", port), None));
    }

    // Check if it's a host:port
    if target.contains(':') {
        return Ok((target.to_string(), None));
    }

    // Assume it's a hostname on port 80
    Ok((format!("{}:80", target), None))
}

/// Start a static file server for directory serving
async fn start_static_server(dir: PathBuf) -> Result<StaticServer> {
    use axum::Router;
    use tower_http::services::ServeDir;

    let app = Router::new().fallback_service(ServeDir::new(&dir));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    cliclack::log::info(format!(
        "Serving static files from {} on {}",
        style(dir.display()).cyan(),
        style(addr).green()
    ))?;

    Ok(StaticServer { addr, _handle: handle })
}

struct StaticServer {
    addr: SocketAddr,
    _handle: tokio::task::JoinHandle<()>,
}

/// Spawn as background process
async fn spawn_background(opts: HttpOptions) -> Result<()> {
    use cliclack::{intro, outro, note};

    intro(style(" dvaar ").on_cyan().black().to_string())?;

    let spinner = cliclack::spinner();
    spinner.start("Starting background tunnel...");

    let session_id = generate_session_id();
    let log_file = logs_dir().join(format!("{}.log", session_id));

    // Build command args (without -d flag, with --no-tui for background mode)
    let mut args = vec!["http".to_string(), opts.target.clone(), "--no-tui".to_string()];

    if let Some(subdomain) = &opts.subdomain {
        args.push("--subdomain".to_string());
        args.push(subdomain.clone());
    }

    if let Some(auth) = &opts.auth {
        args.push("--auth".to_string());
        args.push(auth.clone());
    }

    if let Some(host) = &opts.host_header {
        args.push("--host-header".to_string());
        args.push(host.clone());
    }

    if opts.use_tls {
        args.push("--use-tls".to_string());
    }

    if let Some(port) = opts.inspect_port {
        args.push(format!("--inspect={}", port));
    }

    // Get current executable
    let exe = std::env::current_exe().context("Failed to get current executable")?;

    // Ensure log directory exists
    crate::config::ensure_dirs()?;

    // Open log file
    let log = std::fs::File::create(&log_file).context("Failed to create log file")?;
    let log_err = log.try_clone()?;

    // Spawn child process
    let child = Command::new(&exe)
        .args(&args)
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .stdin(Stdio::null())
        .spawn()
        .context("Failed to spawn background process")?;

    let pid = child.id();

    // Wait a moment and check the log for the URL
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let url = read_url_from_log(&log_file).unwrap_or_else(|| "Connecting...".to_string());

    // Save session
    let session = Session {
        id: session_id.clone(),
        pid,
        command: format!("http {}", opts.target),
        url: url.clone(),
        target: opts.target.clone(),
        started_at: Utc::now(),
    };

    let mut sessions = Sessions::load()?;
    sessions.add(session)?;

    spinner.stop("Background tunnel started");

    // Display session info
    let info = format!(
        "{} {}\n{} {}\n{} {}",
        style("ID:").dim(),
        style(&session_id).cyan(),
        style("URL:").dim(),
        style(&url).green().bold(),
        style("Target:").dim(),
        style(&opts.target).white(),
    );
    note("Tunnel Info", &info)?;

    cliclack::log::info(format!("View logs: {}", style(format!("dvaar logs {}", session_id)).cyan()))?;
    cliclack::log::info(format!("Stop tunnel: {}", style(format!("dvaar stop {}", session_id)).cyan()))?;

    outro("Running in background")?;

    Ok(())
}

fn read_url_from_log(path: &PathBuf) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        if line.contains("https://") && line.contains(".dvaar.") {
            // Extract URL
            let start = line.find("https://")?;
            let end = line[start..]
                .find(|c: char| c.is_whitespace())
                .map(|i| start + i)
                .unwrap_or(line.len());
            return Some(line[start..end].to_string());
        }
    }
    None
}
