//! Uninstall command - remove dvaar from the system

use anyhow::{Context, Result};
use console::style;
use std::fs;

/// Run the uninstall command
pub async fn run(_purge: bool) -> Result<()> {
    use cliclack::{intro, outro, outro_cancel, confirm, log};

    intro(style(" dvaar uninstall ").on_red().white().bold().to_string())?;

    // Get paths
    let binary_path = std::env::current_exe().context("Failed to get binary path")?;
    let config_dir = crate::config::config_dir();

    // Show what will be affected
    println!();
    log::info(format!("Binary location: {}", style(binary_path.display()).cyan()))?;
    if config_dir.exists() {
        log::info(format!("Config directory: {}", style(config_dir.display()).cyan()))?;
    }
    println!();

    // Confirm uninstall
    let proceed = confirm("Are you sure you want to uninstall dvaar?")
        .initial_value(false)
        .interact()?;

    if !proceed {
        outro_cancel("Uninstall cancelled")?;
        return Ok(());
    }

    // Ask about config removal
    let remove_config = if config_dir.exists() {
        println!();
        log::warning("Your config directory contains:")?;
        println!("    • Login credentials (you'll need to re-authenticate)");
        println!("    • Session data and logs");
        println!();

        confirm("Remove config directory? (This will log you out)")
            .initial_value(false)
            .interact()?
    } else {
        false
    };

    // Remove config if requested
    if remove_config {
        log::step("Removing config directory...")?;
        fs::remove_dir_all(&config_dir).context("Failed to remove config directory")?;
        log::success("Config removed")?;
    }

    // Try to remove the binary
    println!();
    log::step("Removing binary...")?;

    #[cfg(unix)]
    let remove_result = {
        if binary_path.starts_with("/usr/local") || binary_path.starts_with("/usr/bin") {
            // Need sudo
            log::info("Root privileges required to remove from system directory")?;
            std::process::Command::new("sudo")
                .args(["rm", "-f", &binary_path.to_string_lossy()])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        } else {
            fs::remove_file(&binary_path).is_ok()
        }
    };

    #[cfg(windows)]
    let remove_result = {
        // On Windows, we can't delete ourselves while running
        // Schedule deletion on next reboot or provide manual instructions
        false
    };

    if remove_result {
        log::success("Binary removed")?;
        println!();
        outro("Dvaar has been uninstalled. Thanks for trying it out!")?;
    } else {
        // Provide manual instructions
        println!();
        log::warning("Could not remove binary automatically")?;
        println!();
        println!("  To complete uninstallation, run:");
        println!();

        #[cfg(unix)]
        {
            if binary_path.starts_with("/usr/local") || binary_path.starts_with("/usr/bin") {
                println!("    {}", style(format!("sudo rm {}", binary_path.display())).green());
            } else {
                println!("    {}", style(format!("rm {}", binary_path.display())).green());
            }
        }

        #[cfg(windows)]
        {
            println!("    {}", style(format!("del \"{}\"", binary_path.display())).green());
        }

        println!();

        if !remove_config && config_dir.exists() {
            println!("  To also remove config:");
            println!("    {}", style(format!("rm -rf {}", config_dir.display())).green());
            println!();
        }

        println!("  If you installed via npm:");
        println!("    {}", style("npm uninstall -g dvaar").green());
        println!();

        outro("Follow the steps above to complete uninstallation")?;
    }

    Ok(())
}
