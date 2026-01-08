//! Login command - authenticate with Dvaar using GitHub Device Flow

use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use console::style;
use serde::Deserialize;
use std::time::Duration;

/// Create a clickable hyperlink for terminals that support OSC 8
fn hyperlink(url: &str, text: &str) -> String {
    format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, text)
}

/// Copy text to clipboard (best effort, doesn't fail if unavailable)
fn copy_to_clipboard(text: &str) {
    #[cfg(target_os = "macos")]
    {
        use std::process::{Command, Stdio};
        let _ = Command::new("pbcopy")
            .stdin(Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(text.as_bytes());
                }
                child.wait()
            });
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::{Command, Stdio};
        // Try xclip first, then xsel
        let _ = Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(text.as_bytes());
                }
                child.wait()
            });
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::{Command, Stdio};
        let _ = Command::new("cmd")
            .args(["/C", &format!("echo {} | clip", text)])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn();
    }
}

const GITHUB_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const GITHUB_ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

/// GitHub Device Code Response
#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

/// GitHub Access Token Response
#[derive(Debug, Deserialize)]
struct AccessTokenResponse {
    access_token: Option<String>,
    token_type: Option<String>,
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// Dvaar Token Exchange Response
#[derive(Debug, Deserialize)]
struct DvaarTokenResponse {
    token: String,
    user: DvaarUser,
}

#[derive(Debug, Deserialize)]
struct DvaarUser {
    id: String,
    email: String,
}

/// Handle login command
pub async fn run(token: Option<String>) -> Result<()> {
    use cliclack::{intro, outro, note, confirm};

    let mut config = Config::load()?;

    intro(style(" dvaar login ").on_cyan().black().to_string())?;

    if let Some(token) = token {
        // Direct token provided
        config.set_token(token);
        config.save()?;
        cliclack::log::success("Token saved successfully")?;
        outro("You're all set!")?;
        return Ok(());
    }

    // GitHub Device Flow
    let spinner = cliclack::spinner();
    spinner.start("Connecting to GitHub...");

    let client = reqwest::Client::new();

    // Step 1: Request device code
    let device_response = request_device_code(&client, &config).await?;
    spinner.stop("Connected to GitHub");

    // Step 2: Display code to user and auto-copy
    copy_to_clipboard(&device_response.user_code);

    let github_link = hyperlink(
        &device_response.verification_uri,
        &device_response.verification_uri
    );

    let code_display = format!(
        "{} {}\n\n{}\n\nPaste it at: {}",
        style("Your code:").white().bold(),
        style(&device_response.user_code).green().bold().bright(),
        style("(Already copied to clipboard!)").dim(),
        style(&github_link).cyan().underlined()
    );
    note("One-Time Code", &code_display)?;

    let should_open = confirm("Open GitHub in your browser?")
        .initial_value(false)
        .interact()?;

    if should_open {
        // Open browser only after user confirms
        if let Err(e) = open::that(&device_response.verification_uri) {
            cliclack::log::warning("Could not open browser automatically")?;
            cliclack::log::info(format!(
                "Please visit: {}",
                hyperlink(&device_response.verification_uri, &device_response.verification_uri)
            ))?;
            tracing::debug!("Failed to open browser: {}", e);
        } else {
            cliclack::log::success("Browser opened - paste your code there")?;
        }
    } else {
        cliclack::log::info(format!(
            "Visit: {}",
            hyperlink(&device_response.verification_uri, &device_response.verification_uri)
        ))?;
    }

    // Step 3: Poll for access token
    let spinner = cliclack::spinner();
    spinner.start("Waiting for authentication...");
    let github_token = poll_for_token(&client, &config, &device_response).await?;
    spinner.stop("Authenticated with GitHub");

    // Step 4: Exchange GitHub token for Dvaar API token
    let spinner = cliclack::spinner();
    spinner.start("Setting up your account...");
    let dvaar_response = exchange_for_dvaar_token(&client, &config, &github_token).await?;
    spinner.stop("Account ready");

    // Step 5: Save token and user info
    config.set_token(dvaar_response.token);
    config.set_user_info(Some(dvaar_response.user.email.clone()), Some("Free".to_string()));
    config.save()?;

    cliclack::log::success(format!("Logged in as {}", style(&dvaar_response.user.email).green()))?;

    println!();
    println!("  {} {}", style("Quick start:").white().bold(), style("dvaar http 3000").green());
    println!();
    println!("  {} {}", style("Common commands:").dim(), "");
    println!("    {}  Create a tunnel to local port", style("dvaar http <port>").cyan());
    println!("    {}        List active tunnels", style("dvaar ls").cyan());
    println!("    {}   View bandwidth usage", style("dvaar usage").cyan());
    println!("    {} Upgrade your plan", style("dvaar upgrade").cyan());
    println!();

    outro("You're all set!")?;

    Ok(())
}

/// Request a device code from GitHub
async fn request_device_code(client: &reqwest::Client, config: &Config) -> Result<DeviceCodeResponse> {
    // Get client_id from server's config endpoint
    let client_id = get_github_client_id(client, config).await?;

    let response = client
        .post(GITHUB_DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id.as_str()),
            ("scope", "user:email"),
        ])
        .send()
        .await
        .context("Failed to request device code")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("GitHub returned {}: {}", status, body));
    }

    response
        .json::<DeviceCodeResponse>()
        .await
        .context("Failed to parse device code response")
}

/// Poll GitHub for access token
async fn poll_for_token(
    client: &reqwest::Client,
    config: &Config,
    device_response: &DeviceCodeResponse,
) -> Result<String> {
    let client_id = get_github_client_id(client, config).await?;
    let interval = Duration::from_secs(device_response.interval.max(5));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(device_response.expires_in);

    loop {
        tokio::time::sleep(interval).await;

        if tokio::time::Instant::now() > deadline {
            return Err(anyhow!("Authentication timed out. Please try again."));
        }

        let response = client
            .post(GITHUB_ACCESS_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id.as_str()),
                ("device_code", device_response.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .context("Failed to poll for token")?;

        let token_response: AccessTokenResponse = response
            .json()
            .await
            .context("Failed to parse token response")?;

        // Check for errors
        if let Some(error) = &token_response.error {
            match error.as_str() {
                "authorization_pending" => {
                    // User hasn't authorized yet, keep polling
                    continue;
                }
                "slow_down" => {
                    // Rate limited, wait longer
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
                "expired_token" => {
                    return Err(anyhow!("Code expired. Please try again."));
                }
                "access_denied" => {
                    return Err(anyhow!("Authorization denied."));
                }
                _ => {
                    let desc = token_response.error_description.unwrap_or_default();
                    return Err(anyhow!("GitHub error: {} - {}", error, desc));
                }
            }
        }

        // Success!
        if let Some(access_token) = token_response.access_token {
            return Ok(access_token);
        }
    }
}

/// Exchange GitHub access token for Dvaar API token
async fn exchange_for_dvaar_token(
    client: &reqwest::Client,
    config: &Config,
    github_token: &str,
) -> Result<DvaarTokenResponse> {
    let url = format!("{}/api/auth/token", config.server_url);

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "github_token": github_token
        }))
        .send()
        .await
        .context("Failed to exchange token")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Server returned {}: {}", status, body));
    }

    response
        .json::<DvaarTokenResponse>()
        .await
        .context("Failed to parse token response")
}

/// Get GitHub client ID from server
async fn get_github_client_id(client: &reqwest::Client, config: &Config) -> Result<String> {
    let url = format!("{}/api/auth/config", config.server_url);

    let response = client
        .get(&url)
        .send()
        .await
        .context("Failed to get auth config from server")?;

    if !response.status().is_success() {
        return Err(anyhow!("Server unavailable. Check your connection."));
    }

    #[derive(Deserialize)]
    struct AuthConfig {
        github_client_id: String,
    }

    let auth_config: AuthConfig = response
        .json()
        .await
        .context("Failed to parse auth config")?;

    Ok(auth_config.github_client_id)
}
