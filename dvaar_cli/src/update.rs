//! Update checker - notifies users of new versions

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const GITHUB_REPO: &str = "strawberry-labs/dvaar";
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60); // 24 hours
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Serialize, Deserialize)]
struct UpdateCache {
    last_check: u64,
    latest_version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
}

/// Get the cache file path
fn cache_path() -> PathBuf {
    crate::config::config_dir().join("update_cache.json")
}

/// Check if we should perform an update check
fn should_check() -> bool {
    let path = cache_path();
    if !path.exists() {
        return true;
    }

    match fs::read_to_string(&path) {
        Ok(content) => {
            if let Ok(cache) = serde_json::from_str::<UpdateCache>(&content) {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                return now - cache.last_check > CHECK_INTERVAL.as_secs();
            }
        }
        Err(_) => {}
    }

    true
}

/// Save the cache
fn save_cache(latest_version: Option<String>) {
    let cache = UpdateCache {
        last_check: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        latest_version,
    };

    if let Ok(content) = serde_json::to_string(&cache) {
        let _ = fs::write(cache_path(), content);
    }
}

/// Get cached latest version
fn get_cached_version() -> Option<String> {
    let path = cache_path();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(cache) = serde_json::from_str::<UpdateCache>(&content) {
            return cache.latest_version;
        }
    }
    None
}

/// Fetch latest version from GitHub (async)
async fn fetch_latest_version() -> Result<String> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("dvaar-cli")
        .build()?;

    let response: GithubRelease = client.get(&url).send().await?.json().await?;

    let version = response.tag_name.trim_start_matches('v').to_string();
    Ok(version)
}

/// Compare versions (returns true if latest > current)
fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse_version = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|s| s.parse().ok())
            .collect()
    };

    let latest_parts = parse_version(latest);
    let current_parts = parse_version(current);

    for i in 0..latest_parts.len().max(current_parts.len()) {
        let l = latest_parts.get(i).unwrap_or(&0);
        let c = current_parts.get(i).unwrap_or(&0);
        if l > c {
            return true;
        }
        if l < c {
            return false;
        }
    }

    false
}

/// Check for updates and print message if available
/// This is non-blocking and won't fail the main command
pub async fn check_for_updates() {
    // Skip in CI or tests
    if std::env::var("CI").is_ok() || std::env::var("DVAAR_NO_UPDATE_CHECK").is_ok() {
        return;
    }

    // Check from cache first
    if let Some(cached_version) = get_cached_version() {
        if is_newer_version(&cached_version, CURRENT_VERSION) {
            print_update_message(&cached_version);
        }
    }

    // Do a background check if needed
    if should_check() {
        tokio::spawn(async {
            if let Ok(latest) = fetch_latest_version().await {
                save_cache(Some(latest));
            } else {
                save_cache(None);
            }
        });
    }
}

/// Check for updates (blocking, for explicit update check command)
pub async fn check_for_updates_blocking() -> Result<Option<String>> {
    let latest = fetch_latest_version().await?;
    save_cache(Some(latest.clone()));

    if is_newer_version(&latest, CURRENT_VERSION) {
        Ok(Some(latest))
    } else {
        Ok(None)
    }
}

/// Print update available message
fn print_update_message(latest_version: &str) {
    eprintln!();
    eprintln!("\x1b[33m╭─────────────────────────────────────────────╮\x1b[0m");
    eprintln!(
        "\x1b[33m│\x1b[0m  A new version of dvaar is available: \x1b[32m{}\x1b[0m  \x1b[33m│\x1b[0m",
        latest_version
    );
    eprintln!("\x1b[33m│\x1b[0m  You have: {}                              \x1b[33m│\x1b[0m", CURRENT_VERSION);
    eprintln!("\x1b[33m│\x1b[0m                                             \x1b[33m│\x1b[0m");
    eprintln!("\x1b[33m│\x1b[0m  Update with:                               \x1b[33m│\x1b[0m");
    eprintln!("\x1b[33m│\x1b[0m  \x1b[36mcurl -sSL https://dvaar.io/install.sh | bash\x1b[0m \x1b[33m│\x1b[0m");
    eprintln!("\x1b[33m╰─────────────────────────────────────────────╯\x1b[0m");
    eprintln!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("1.0.1", "1.0.0"));
        assert!(is_newer_version("1.1.0", "1.0.0"));
        assert!(is_newer_version("2.0.0", "1.9.9"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("1.0.0", "1.0.1"));
        assert!(!is_newer_version("0.9.0", "1.0.0"));
    }
}
