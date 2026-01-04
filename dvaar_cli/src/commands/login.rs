//! Login command - authenticate with Dvaar using GitHub Device Flow

use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

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
    let mut config = Config::load()?;

    if let Some(token) = token {
        // Direct token provided
        config.set_token(token);
        config.save()?;
        println!("Token saved successfully.");
        return Ok(());
    }

    // GitHub Device Flow
    println!();
    println!("Authenticating with GitHub...");
    println!();

    let client = reqwest::Client::new();

    // Step 1: Request device code
    let device_response = request_device_code(&client, &config).await?;

    // Step 2: Display code to user
    println!("! First, copy your one-time code: {}", device_response.user_code);
    println!();
    println!("Press Enter to open {} in your browser...", device_response.verification_uri);

    // Wait for Enter key
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    // Open browser
    if let Err(e) = open::that(&device_response.verification_uri) {
        println!("Could not open browser automatically.");
        println!("Please open: {}", device_response.verification_uri);
        tracing::debug!("Failed to open browser: {}", e);
    }

    println!();
    println!("Waiting for authentication...");

    // Step 3: Poll for access token
    let github_token = poll_for_token(&client, &config, &device_response).await?;

    // Step 4: Exchange GitHub token for Dvaar API token
    println!("Exchanging token...");
    let dvaar_response = exchange_for_dvaar_token(&client, &config, &github_token).await?;

    // Step 5: Save token
    config.set_token(dvaar_response.token);
    config.save()?;

    println!();
    println!("Logged in as {}", dvaar_response.user.email);
    println!();

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
