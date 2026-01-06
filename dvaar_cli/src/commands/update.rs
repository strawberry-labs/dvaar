//! Update command - update dvaar to the latest version

use anyhow::{Context, Result};
use std::process::Command;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Run the update command
pub async fn run() -> Result<()> {
    println!("Checking for updates...");
    println!();

    // Check for latest version
    match crate::update::check_for_updates_blocking().await {
        Ok(Some(latest)) => {
            println!("New version available: {} (current: {})", latest, CURRENT_VERSION);
            println!();
            do_update()?;
        }
        Ok(None) => {
            println!("You're already on the latest version ({})", CURRENT_VERSION);
        }
        Err(e) => {
            println!("Failed to check for updates: {}", e);
            println!();
            println!("You can manually update by running:");
            print_manual_update_instructions();
        }
    }

    Ok(())
}

fn do_update() -> Result<()> {
    println!("Updating dvaar...");
    println!();

    #[cfg(unix)]
    {
        // Use the install script
        let status = Command::new("sh")
            .arg("-c")
            .arg("curl -fsSL https://dvaar.io/install.sh | bash")
            .status()
            .context("Failed to run update")?;

        if !status.success() {
            println!();
            println!("Update failed. Try manually:");
            print_manual_update_instructions();
        }
    }

    #[cfg(windows)]
    {
        // Use PowerShell install script
        let status = Command::new("powershell")
            .args([
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "iwr -useb https://dvaar.io/install.ps1 | iex",
            ])
            .status()
            .context("Failed to run update")?;

        if !status.success() {
            println!();
            println!("Update failed. Try manually:");
            print_manual_update_instructions();
        }
    }

    Ok(())
}

fn print_manual_update_instructions() {
    #[cfg(unix)]
    {
        println!("  curl -fsSL https://dvaar.io/install.sh | bash");
    }

    #[cfg(windows)]
    {
        println!("  iwr -useb https://dvaar.io/install.ps1 | iex");
    }

    println!();
    println!("Or via npm:");
    println!("  npm install -g dvaar");
}
