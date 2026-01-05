//! CLI configuration management

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Get the configuration directory path
pub fn config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dvaar")
    }

    #[cfg(not(target_os = "windows"))]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".dvaar")
    }
}

/// Get the config file path
pub fn config_file() -> PathBuf {
    config_dir().join("config.yml")
}

/// Get the sessions file path
pub fn sessions_file() -> PathBuf {
    config_dir().join("sessions.json")
}

/// Get the logs directory
pub fn logs_dir() -> PathBuf {
    config_dir().join("logs")
}

/// Ensure all config directories exist
pub fn ensure_dirs() -> Result<()> {
    let config = config_dir();
    let logs = logs_dir();

    fs::create_dir_all(&config).context("Failed to create config directory")?;
    fs::create_dir_all(&logs).context("Failed to create logs directory")?;

    Ok(())
}

/// Main configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Authentication token
    pub authtoken: Option<String>,

    /// Server URL (default: https://api.dvaar.io)
    #[serde(default = "default_server_url")]
    pub server_url: String,
}

fn default_server_url() -> String {
    "https://api.dvaar.io".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            authtoken: None,
            server_url: default_server_url(),
        }
    }
}

impl Config {
    /// Load config from file
    pub fn load() -> Result<Self> {
        let path = config_file();

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path).context("Failed to read config file")?;
        let config: Config = serde_yaml::from_str(&content).context("Failed to parse config file")?;

        Ok(config)
    }

    /// Save config to file
    pub fn save(&self) -> Result<()> {
        ensure_dirs()?;
        let path = config_file();
        let content = serde_yaml::to_string(self).context("Failed to serialize config")?;
        fs::write(&path, content).context("Failed to write config file")?;
        Ok(())
    }

    /// Check if authenticated
    pub fn is_authenticated(&self) -> bool {
        self.authtoken.is_some()
    }

    /// Get auth token or error
    pub fn require_auth(&self) -> Result<&str> {
        self.authtoken
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Not logged in. Run `dvaar login` first."))
    }

    /// Set auth token
    pub fn set_token(&mut self, token: String) {
        self.authtoken = Some(token);
    }

    /// Get WebSocket URL from server URL
    pub fn websocket_url(&self) -> String {
        let ws_scheme = if self.server_url.starts_with("https://") {
            "wss"
        } else {
            "ws"
        };
        let host = self
            .server_url
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        format!("{}://{}", ws_scheme, host)
    }
}

/// Session information for background processes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session ID
    pub id: String,

    /// Process ID
    pub pid: u32,

    /// Command that was run (e.g., "http 8080")
    pub command: String,

    /// Public URL
    pub url: String,

    /// Local target
    pub target: String,

    /// When the session was started
    pub started_at: DateTime<Utc>,
}

/// Sessions registry
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Sessions {
    pub sessions: Vec<Session>,
}

impl Sessions {
    /// Load sessions from file
    pub fn load() -> Result<Self> {
        let path = sessions_file();

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path).context("Failed to read sessions file")?;
        let sessions: Sessions =
            serde_json::from_str(&content).context("Failed to parse sessions file")?;

        Ok(sessions)
    }

    /// Save sessions to file
    pub fn save(&self) -> Result<()> {
        ensure_dirs()?;
        let path = sessions_file();
        let content = serde_json::to_string_pretty(self).context("Failed to serialize sessions")?;
        fs::write(&path, content).context("Failed to write sessions file")?;
        Ok(())
    }

    /// Add a session
    pub fn add(&mut self, session: Session) -> Result<()> {
        self.sessions.push(session);
        self.save()
    }

    /// Remove a session by ID
    pub fn remove(&mut self, id: &str) -> Result<Option<Session>> {
        let idx = self.sessions.iter().position(|s| s.id == id);
        let removed = idx.map(|i| self.sessions.remove(i));
        self.save()?;
        Ok(removed)
    }

    /// Find a session by ID
    pub fn find(&self, id: &str) -> Option<&Session> {
        self.sessions.iter().find(|s| s.id == id || s.id.starts_with(id))
    }

    /// Get all sessions
    pub fn all(&self) -> &[Session] {
        &self.sessions
    }
}

/// Generate a short random ID
pub fn generate_session_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 4] = rng.gen();
    hex::encode(&bytes)
}

// Add hex encoding since we use it
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
