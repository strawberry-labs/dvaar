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

/// Detected package manager type
#[derive(Debug, Clone, Copy, PartialEq)]
enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Binary,
}

/// Detect how dvaar was installed
fn detect_install_method() -> PackageManager {
    if let Ok(exe_path) = std::env::current_exe() {
        let path_str = exe_path.to_string_lossy().to_lowercase();

        // Check for pnpm first (more specific)
        if path_str.contains("pnpm") {
            return PackageManager::Pnpm;
        }

        // Check for yarn
        if path_str.contains("yarn") || path_str.contains(".yarn") {
            return PackageManager::Yarn;
        }

        // Check for npm/node_modules
        if path_str.contains("node_modules") || path_str.contains("/npm/") || path_str.contains("\\npm\\") {
            return PackageManager::Npm;
        }

        // Check for npm global install paths
        #[cfg(unix)]
        {
            if path_str.contains("/lib/node_modules/") {
                return PackageManager::Npm;
            }
        }
    }

    PackageManager::Binary
}

fn do_update() -> Result<()> {
    println!("Updating dvaar...");
    println!();

    let install_method = detect_install_method();

    match install_method {
        PackageManager::Pnpm => {
            println!("Detected pnpm installation. Updating via pnpm...");
            println!();

            let status = Command::new("pnpm")
                .args(["add", "-g", "dvaar@latest"])
                .status()
                .context("Failed to run pnpm update. Make sure pnpm is in your PATH.")?;

            if !status.success() {
                println!();
                println!("pnpm update failed. Try manually:");
                println!("  pnpm add -g dvaar@latest");
                println!();
                println!("Or switch to binary install:");
                print_binary_install_instructions();
            } else {
                println!();
                println!("Update complete! Run 'dvaar --version' to verify.");
            }

            return Ok(());
        }
        PackageManager::Yarn => {
            println!("Detected yarn installation. Updating via yarn...");
            println!();

            let status = Command::new("yarn")
                .args(["global", "add", "dvaar@latest"])
                .status()
                .context("Failed to run yarn update. Make sure yarn is in your PATH.")?;

            if !status.success() {
                println!();
                println!("yarn update failed. Try manually:");
                println!("  yarn global add dvaar@latest");
                println!();
                println!("Or switch to binary install:");
                print_binary_install_instructions();
            } else {
                println!();
                println!("Update complete! Run 'dvaar --version' to verify.");
            }

            return Ok(());
        }
        PackageManager::Npm => {
            println!("Detected npm installation. Updating via npm...");
            println!();

            let status = Command::new("npm")
                .args(["install", "-g", "dvaar@latest"])
                .status()
                .context("Failed to run npm update. Make sure npm is in your PATH.")?;

            if !status.success() {
                println!();
                println!("npm update failed. Try manually:");
                println!("  npm install -g dvaar@latest");
                println!();
                println!("Or switch to binary install:");
                print_binary_install_instructions();
            } else {
                println!();
                println!("Update complete! Run 'dvaar --version' to verify.");
            }

            return Ok(());
        }
        PackageManager::Binary => {
            // Fall through to binary install below
        }
    }

    // Binary installation - use install script
    #[cfg(unix)]
    {
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

fn print_binary_install_instructions() {
    #[cfg(unix)]
    {
        println!("  curl -fsSL https://dvaar.io/install.sh | bash");
    }

    #[cfg(windows)]
    {
        println!("  iwr -useb https://dvaar.io/install.ps1 | iex");
    }
}

fn print_manual_update_instructions() {
    print_binary_install_instructions();
    println!();
    println!("Or via npm:");
    println!("  npm install -g dvaar@latest");
}
