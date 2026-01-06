//! Billing commands (upgrade, usage, billing portal)

use crate::config::Config;
use anyhow::Result;
use console::style;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct CheckoutRequest {
    plan: String,
}

#[derive(Debug, Deserialize)]
struct CheckoutResponse {
    checkout_url: String,
}

#[derive(Debug, Deserialize)]
struct PortalResponse {
    portal_url: String,
}

/// Create a clickable hyperlink for terminals that support OSC 8
fn hyperlink(url: &str, text: &str) -> String {
    format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, text)
}


/// Show usage statistics
pub async fn usage() -> Result<()> {
    use cliclack::{intro, outro, log};

    let config = Config::load()?;
    let token = config.require_auth()?;

    intro("dvaar usage")?;

    let spinner = cliclack::spinner();
    spinner.start("Fetching usage data...");

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/api/usage", config.server_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        spinner.error(format!("Failed to fetch usage: {} - {}", status, text));
        return Ok(());
    }

    let data: serde_json::Value = response.json().await?;
    spinner.stop("Usage data retrieved");

    let plan = data["plan"].as_str().unwrap_or("free");
    let bandwidth = data["bandwidth_bytes"].as_u64().unwrap_or(0);
    let limit = data["bandwidth_limit"].as_str().unwrap_or("unlimited");

    log::info(format!("Plan: {}", capitalize(plan)))?;
    log::info(format!("Bandwidth Used: {}", format_bytes(bandwidth)))?;
    log::info(format!("Bandwidth Limit: {}", limit))?;

    outro("Done")?;

    Ok(())
}

/// Upgrade to a paid plan
pub async fn upgrade(plan: Option<String>) -> Result<()> {
    use cliclack::{intro, outro, outro_cancel, log, note};

    let config = Config::load()?;
    let token = config.require_auth()?;

    intro("dvaar upgrade")?;

    // If no plan specified, show interactive selection
    let plan = match plan {
        Some(p) => p,
        None => {
            match select_plan()? {
                Some(p) => p,
                None => {
                    outro_cancel("Upgrade cancelled")?;
                    return Ok(());
                }
            }
        }
    };

    let spinner = cliclack::spinner();
    spinner.start(format!("Creating checkout session for {} plan...", plan));

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/billing/checkout", config.server_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&CheckoutRequest { plan: plan.clone() })
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        spinner.error(format!("Failed: {} - {}", status, text));
        return Ok(());
    }

    let checkout: CheckoutResponse = response.json().await?;
    spinner.stop("Checkout session created");

    println!();
    log::info("Opening checkout page in your browser...")?;
    println!();
    println!("  {}", style(hyperlink(&checkout.checkout_url, &checkout.checkout_url)).white());
    println!();

    if let Err(e) = open::that(&checkout.checkout_url) {
        tracing::warn!("Failed to open browser: {}", e);
        log::warning("Could not open browser automatically. Please visit the URL above.")?;
    }

    outro("Complete payment in your browser to activate your plan")?;

    Ok(())
}

/// Interactive plan selection
fn select_plan() -> Result<Option<String>> {
    use cliclack::{select, note};
    use console::style;

    // Show pricing table
    let pricing_table = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        "┌─────────────┬─────────────┬─────────────┬─────────────┐",
        "│   Feature   │    Free     │   Hobby     │     Pro     │",
        "├─────────────┼─────────────┼─────────────┼─────────────┤",
        "│ Price       │     $0      │   $5/mo     │   $15/mo    │",
        "├─────────────┼─────────────┼─────────────┼─────────────┤",
        "│ Tunnels/hr  │      5      │     20      │    100      │",
        "│ Requests/m  │     60      │    600      │   3000      │",
        "├─────────────┼─────────────┼─────────────┼─────────────┤",
        "│ Custom sub  │      ✗      │     ✓       │     ✓       │",
        "│ Reserved    │      ✗      │     ✓       │     ✓       │",
        "└─────────────┴─────────────┴─────────────┴─────────────┘",
    );

    note("Pricing", &pricing_table)?;
    let pricing_url = "https://dvaar.io/#pricing";
    cliclack::log::info(format!("Full details: {}", style(hyperlink(pricing_url, pricing_url)).cyan().underlined()))?;

    let selection: &str = select("Select a plan to upgrade")
        .item("hobby", "Hobby - $5/month", "20 tunnels/hr, 600 req/min, custom subdomains")
        .item("pro", "Pro - $15/month", "100 tunnels/hr, 3000 req/min, 5 team seats")
        .interact()?;

    Ok(Some(selection.to_string()))
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

/// Open billing portal to manage subscription and view invoices
pub async fn portal() -> Result<()> {
    use cliclack::{intro, outro, log};

    let config = Config::load()?;
    let token = config.require_auth()?;

    intro(style(" dvaar billing ").on_cyan().black().to_string())?;

    let spinner = cliclack::spinner();
    spinner.start("Creating billing portal session...");

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/billing/portal", config.server_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();

        if status.as_u16() == 400 && text.contains("no active subscription") {
            spinner.stop("No active subscription");
            println!();
            log::info("You're on the free plan. Upgrade to access billing portal.")?;
            println!();
            println!("  Run {} to upgrade your plan", style("dvaar upgrade").green());
            println!();
            outro("Done")?;
            return Ok(());
        }

        spinner.error(format!("Failed: {} - {}", status, text));
        return Ok(());
    }

    let portal: PortalResponse = response.json().await?;
    spinner.stop("Portal session created");

    println!();
    log::info("Opening billing portal in your browser...")?;
    println!();
    println!("  {}", style(hyperlink(&portal.portal_url, &portal.portal_url)).white());
    println!();

    if let Err(e) = open::that(&portal.portal_url) {
        tracing::warn!("Failed to open browser: {}", e);
        log::warning("Could not open browser automatically. Please visit the URL above.")?;
    }

    println!();
    println!("  In the billing portal you can:");
    println!("    • View and download invoices");
    println!("    • Update payment method");
    println!("    • Cancel or change subscription");
    println!();

    outro("Done")?;

    Ok(())
}
