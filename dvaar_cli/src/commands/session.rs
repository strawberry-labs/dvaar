//! Session management commands (ls, stop, logs)

use crate::config::{logs_dir, Sessions};
use anyhow::{Context, Result};
use std::io::{BufRead, BufReader};

/// List all active sessions
pub async fn list() -> Result<()> {
    let sessions = Sessions::load()?;
    let sessions = sessions.all();

    if sessions.is_empty() {
        println!("No active sessions.");
        println!();
        println!("Start a tunnel with: dvaar http <PORT> -d");
        return Ok(());
    }

    // Print header
    println!(
        "{:<10} {:<30} {:<40} {:<10}",
        "ID", "COMMAND", "URL", "STARTED"
    );
    println!("{}", "-".repeat(90));

    for session in sessions {
        // Check if process is still running
        let status = if is_process_running(session.pid) {
            "running"
        } else {
            "stopped"
        };

        let started = session
            .started_at
            .format("%Y-%m-%d %H:%M")
            .to_string();

        println!(
            "{:<10} {:<30} {:<40} {:<10}",
            session.id,
            truncate(&session.command, 28),
            truncate(&session.url, 38),
            started
        );

        if status == "stopped" {
            println!("  ^ Process no longer running. Use `dvaar stop {}` to clean up.", session.id);
        }
    }

    Ok(())
}

/// Stop a session by ID
pub async fn stop(id: &str) -> Result<()> {
    let mut sessions = Sessions::load()?;

    let session = sessions
        .find(id)
        .ok_or_else(|| anyhow::anyhow!("Session not found: {}", id))?
        .clone();

    // Kill the process
    if is_process_running(session.pid) {
        kill_process(session.pid)?;
        println!("Stopped tunnel: {}", session.url);
    } else {
        println!("Process was already stopped.");
    }

    // Remove from sessions
    sessions.remove(&session.id)?;

    // Optionally clean up log file
    let log_file = logs_dir().join(format!("{}.log", session.id));
    if log_file.exists() {
        println!("Log file: {:?}", log_file);
        println!("(You can delete it manually if no longer needed)");
    }

    Ok(())
}

/// Tail logs for a session
pub async fn logs(id: &str, follow: bool) -> Result<()> {
    let sessions = Sessions::load()?;

    let session = sessions
        .find(id)
        .ok_or_else(|| anyhow::anyhow!("Session not found: {}", id))?;

    let log_file = logs_dir().join(format!("{}.log", session.id));

    if !log_file.exists() {
        println!("No log file found for session: {}", id);
        return Ok(());
    }

    if follow {
        // Follow mode - tail the file
        tail_follow(&log_file).await?;
    } else {
        // Just read the whole file
        let content = std::fs::read_to_string(&log_file)?;
        print!("{}", content);
    }

    Ok(())
}

/// Check if a process is running
fn is_process_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        use nix::sys::signal::kill;
        use nix::unistd::Pid;

        kill(Pid::from_raw(pid as i32), None).is_ok()
    }

    #[cfg(windows)]
    {
        // On Windows, try to open the process
        unsafe {
            let handle = windows::Win32::System::Threading::OpenProcess(
                windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                pid,
            );
            if handle.is_ok() {
                let _ = windows::Win32::Foundation::CloseHandle(handle.unwrap());
                true
            } else {
                false
            }
        }
    }
}

/// Kill a process
fn kill_process(pid: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        kill(Pid::from_raw(pid as i32), Signal::SIGTERM)
            .context("Failed to send SIGTERM")?;
    }

    #[cfg(windows)]
    {
        std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output()
            .context("Failed to kill process")?;
    }

    Ok(())
}

/// Tail a file and follow new content
async fn tail_follow(path: &std::path::Path) -> Result<()> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    // First, print existing content
    for line in reader.lines() {
        println!("{}", line?);
    }

    // Then follow new content
    println!("--- Following log (Ctrl+C to stop) ---");

    let mut last_pos = std::fs::metadata(path)?.len();

    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let current_size = std::fs::metadata(path)?.len();

        if current_size > last_pos {
            let file = std::fs::File::open(path)?;
            let mut reader = BufReader::new(file);

            // Seek to last position
            use std::io::{Seek, SeekFrom};
            reader.seek(SeekFrom::Start(last_pos))?;

            for line in reader.lines() {
                println!("{}", line?);
            }

            last_pos = current_size;
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}
