//! Billing commands (upgrade, usage)

use crate::config::Config;
use anyhow::Result;

/// Show usage statistics
pub async fn usage() -> Result<()> {
    let config = Config::load()?;
    let token = config.require_auth()?;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/api/usage", config.server_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("Failed to fetch usage: {} - {}", status, text);
    }

    let data: serde_json::Value = response.json().await?;

    let plan = data["plan"].as_str().unwrap_or("free");
    let bandwidth = data["bandwidth_bytes"].as_u64().unwrap_or(0);
    let limit = data["bandwidth_limit"].as_str().unwrap_or("unlimited");

    println!("Plan: {}", capitalize(plan));
    println!();
    println!("Bandwidth Usage:");
    println!("  Used:  {}", format_bytes(bandwidth));
    println!("  Limit: {}", limit);

    Ok(())
}

/// Open upgrade page
pub async fn upgrade() -> Result<()> {
    let config = Config::load()?;
    let _ = config.require_auth()?;

    let upgrade_url = format!("{}/upgrade", config.server_url.replace("/api", ""));

    println!("Opening upgrade page...");
    println!("If browser doesn't open, visit: {}", upgrade_url);

    if let Err(e) = open::that(&upgrade_url) {
        tracing::warn!("Failed to open browser: {}", e);
    }

    Ok(())
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}
