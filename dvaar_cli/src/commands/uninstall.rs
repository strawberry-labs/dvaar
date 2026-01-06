//! Uninstall command - remove dvaar from the system

use anyhow::{Context, Result};
use std::fs;
use std::io::{self, Write};

/// Run the uninstall command
pub async fn run(purge: bool) -> Result<()> {
    println!("Uninstalling dvaar...");
    println!();

    // Confirm
    if !confirm_uninstall()? {
        println!("Uninstall cancelled.");
        return Ok(());
    }

    // Find the binary location
    let binary_path = std::env::current_exe().context("Failed to get binary path")?;
    println!("Binary location: {}", binary_path.display());

    // Remove config if purge is set
    if purge {
        let config_dir = crate::config::config_dir();
        if config_dir.exists() {
            println!("Removing config directory: {}", config_dir.display());
            fs::remove_dir_all(&config_dir).context("Failed to remove config directory")?;
            println!("  Config removed.");
        }
    }

    // Provide instructions for binary removal
    // (We can't delete ourselves while running on all platforms)
    println!();
    println!("To complete uninstallation, remove the binary:");
    println!();

    #[cfg(unix)]
    {
        let path_str = binary_path.display();
        if binary_path.starts_with("/usr/local") {
            println!("  sudo rm {}", path_str);
        } else {
            println!("  rm {}", path_str);
        }
    }

    #[cfg(windows)]
    {
        println!("  del \"{}\"", binary_path.display());
    }

    println!();

    if !purge {
        let config_dir = crate::config::config_dir();
        if config_dir.exists() {
            println!("Config directory preserved at: {}", config_dir.display());
            println!("To remove it, run: dvaar uninstall --purge");
            println!("Or manually: rm -rf {}", config_dir.display());
            println!();
        }
    }

    // If installed via package manager, mention that
    println!("If you installed via npm:");
    println!("  npm uninstall -g dvaar");
    println!();

    println!("Thanks for using Dvaar!");

    Ok(())
}

fn confirm_uninstall() -> Result<bool> {
    print!("Are you sure you want to uninstall dvaar? [y/N] ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().eq_ignore_ascii_case("y") || input.trim().eq_ignore_ascii_case("yes"))
}
