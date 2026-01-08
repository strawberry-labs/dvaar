//! Update checker - notifies users of new versions

use anyhow::Result;
use console::style;
use serde::Deserialize;
use std::time::Duration;

const GITHUB_REPO: &str = "strawberry-labs/dvaar";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
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

/// Check for updates and prompt user if available
/// Always checks GitHub releases API (fast, 5s timeout)
pub async fn check_for_updates() {
    // Skip in CI or tests
    if std::env::var("CI").is_ok() || std::env::var("DVAAR_NO_UPDATE_CHECK").is_ok() {
        return;
    }

    if let Ok(latest) = fetch_latest_version().await {
        if is_newer_version(&latest, CURRENT_VERSION) {
            prompt_for_update(&latest).await;
        }
    }
}

/// Check for updates (for explicit update check command)
pub async fn check_for_updates_blocking() -> Result<Option<String>> {
    let latest = fetch_latest_version().await?;

    if is_newer_version(&latest, CURRENT_VERSION) {
        Ok(Some(latest))
    } else {
        Ok(None)
    }
}

/// Prompt user to update or skip
async fn prompt_for_update(latest_version: &str) {
    use cliclack::{intro, select};

    // Show update notice
    let _ = intro(style(" Update Available ").on_yellow().black().to_string());

    println!();
    println!(
        "  {} {}  →  {}",
        style("dvaar").white().bold(),
        style(CURRENT_VERSION).dim(),
        style(latest_version).green().bold()
    );
    println!();

    let choice = select("What would you like to do?")
        .item("update", "Update now", "Run dvaar update")
        .item("skip", "Skip for now", "Continue without updating")
        .interact();

    match choice {
        Ok("update") => {
            println!();
            // Run dvaar update command
            let status = std::process::Command::new(std::env::current_exe().unwrap_or_else(|_| "dvaar".into()))
                .arg("update")
                .status();

            match status {
                Ok(s) if s.success() => {
                    // Exit after successful update
                    std::process::exit(0);
                }
                Ok(_) => {
                    eprintln!("{}", style("Update failed. Continuing with current version...").yellow());
                }
                Err(e) => {
                    eprintln!("{} {}", style("Could not run update:").red(), e);
                }
            }
        }
        Ok("skip") | Err(_) => {
            // Continue without updating
            println!();
        }
        _ => {}
    }
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
