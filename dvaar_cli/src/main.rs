//! Dvaar CLI - Expose local services to the internet
//!
//! Usage:
//!   dvaar login [TOKEN]         Authenticate with Dvaar
//!   dvaar http <TARGET>         Create an HTTP tunnel
//!   dvaar ls                    List active tunnels
//!   dvaar stop <ID>             Stop a tunnel
//!   dvaar logs <ID>             View tunnel logs
//!   dvaar usage                 View bandwidth usage
//!   dvaar upgrade               Upgrade your plan

mod commands;
mod config;
mod tunnel;
mod update;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "dvaar")]
#[command(author = "Dvaar Team")]
#[command(version)]
#[command(about = "Expose local services to the internet", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate with Dvaar
    Login {
        /// Authentication token (if not provided, opens browser)
        token: Option<String>,
    },

    /// Create an HTTP tunnel
    Http {
        /// Target to tunnel to (port, host:port, URL, or directory path)
        target: String,

        /// Request a specific subdomain (e.g., -s myapp → myapp.dvaar.app)
        #[arg(short = 's', long = "subdomain")]
        subdomain: Option<String>,

        /// Enable basic authentication (format: user:password)
        #[arg(long)]
        auth: Option<String>,

        /// Override the Host header sent to upstream
        #[arg(long)]
        host_header: Option<String>,

        /// Run in background (daemon mode)
        #[arg(short = 'd', long)]
        detach: bool,

        /// Use HTTPS for upstream connection
        #[arg(long)]
        use_tls: bool,
    },

    /// List active tunnels
    Ls,

    /// Stop a tunnel
    Stop {
        /// Session ID (or prefix)
        id: String,
    },

    /// View tunnel logs
    Logs {
        /// Session ID (or prefix)
        id: String,

        /// Follow log output
        #[arg(short, long)]
        follow: bool,
    },

    /// View bandwidth usage
    Usage,

    /// Upgrade your plan
    Upgrade {
        /// Plan to upgrade to (hobby or pro)
        #[arg(value_parser = ["hobby", "pro"])]
        plan: Option<String>,
    },

    /// Update dvaar to the latest version
    Update,

    /// Uninstall dvaar from your system
    Uninstall {
        /// Also remove configuration files
        #[arg(long)]
        purge: bool,
    },

    /// Open billing portal to manage subscription and view invoices
    Billing,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "warn" };
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{},dvaar_cli=info", log_level).into()),
        )
        .with(tracing_subscriber::fmt::layer().without_time())
        .init();

    // Ensure config directories exist
    config::ensure_dirs()?;

    // Check for updates (non-blocking)
    update::check_for_updates().await;

    // Handle commands
    match cli.command {
        Commands::Login { token } => {
            commands::login::run(token).await?;
        }

        Commands::Http {
            target,
            subdomain,
            auth,
            host_header,
            detach,
            use_tls,
        } => {
            let opts = commands::http::HttpOptions {
                target,
                subdomain,
                auth,
                host_header,
                detach,
                use_tls,
            };
            commands::http::run(opts).await?;
        }

        Commands::Ls => {
            commands::session::list().await?;
        }

        Commands::Stop { id } => {
            commands::session::stop(&id).await?;
        }

        Commands::Logs { id, follow } => {
            commands::session::logs(&id, follow).await?;
        }

        Commands::Usage => {
            commands::billing::usage().await?;
        }

        Commands::Upgrade { plan } => {
            commands::billing::upgrade(plan).await?;
        }

        Commands::Update => {
            commands::update::run().await?;
        }

        Commands::Uninstall { purge } => {
            commands::uninstall::run(purge).await?;
        }

        Commands::Billing => {
            commands::billing::portal().await?;
        }
    }

    Ok(())
}
