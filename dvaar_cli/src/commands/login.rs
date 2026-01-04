//! Login command - authenticate with Dvaar

use crate::config::Config;
use anyhow::{Context, Result};
use axum::{
    extract::Query,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::oneshot;

/// Handle login command
pub async fn run(token: Option<String>) -> Result<()> {
    let mut config = Config::load()?;

    if let Some(token) = token {
        // Direct token provided
        config.set_token(token);
        config.save()?;
        println!("Login successful! Token saved.");
        return Ok(());
    }

    // Browser-based OAuth flow
    println!("Starting browser authentication...");

    // Find available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .context("Failed to bind to local port")?;
    let port = listener.local_addr()?.port();

    let redirect_uri = format!("http://localhost:{}/callback", port);
    let auth_url = format!(
        "{}/api/auth/cli?redirect_uri={}",
        config.server_url,
        urlencoding::encode(&redirect_uri)
    );

    // Create channel for receiving token
    let (tx, rx) = oneshot::channel::<String>();
    let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

    // Build callback server
    let app = Router::new().route(
        "/callback",
        get({
            let tx = tx.clone();
            move |query: Query<CallbackQuery>| {
                let tx = tx.clone();
                async move { handle_callback(query, tx).await }
            }
        }),
    );

    // Open browser
    println!("Opening browser to authenticate...");
    println!("If browser doesn't open, visit: {}", auth_url);

    if let Err(e) = open::that(&auth_url) {
        tracing::warn!("Failed to open browser: {}", e);
    }

    // Start server
    let server = axum::serve(listener, app);

    // Wait for callback or timeout
    let timeout = tokio::time::Duration::from_secs(300); // 5 minutes

    tokio::select! {
        result = rx => {
            match result {
                Ok(token) => {
                    config.set_token(token);
                    config.save()?;
                    println!("\nLogin successful! Token saved.");
                }
                Err(_) => {
                    println!("\nLogin cancelled.");
                }
            }
        }
        _ = tokio::time::sleep(timeout) => {
            println!("\nLogin timed out. Please try again.");
        }
        _ = server => {
            // Server stopped unexpectedly
        }
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    token: Option<String>,
    error: Option<String>,
}

async fn handle_callback(
    Query(query): Query<CallbackQuery>,
    tx: Arc<tokio::sync::Mutex<Option<oneshot::Sender<String>>>>,
) -> impl IntoResponse {
    if let Some(error) = query.error {
        return Html(format!(
            r#"
            <!DOCTYPE html>
            <html>
            <head><title>Dvaar Login</title></head>
            <body style="font-family: sans-serif; text-align: center; padding: 50px;">
                <h1>Login Failed</h1>
                <p>Error: {}</p>
                <p>You can close this window.</p>
            </body>
            </html>
            "#,
            error
        ));
    }

    if let Some(token) = query.token {
        if let Some(sender) = tx.lock().await.take() {
            let _ = sender.send(token);
        }

        Html(
            r#"
            <!DOCTYPE html>
            <html>
            <head><title>Dvaar Login</title></head>
            <body style="font-family: sans-serif; text-align: center; padding: 50px;">
                <h1>Login Successful!</h1>
                <p>You can close this window and return to the terminal.</p>
                <script>setTimeout(() => window.close(), 2000);</script>
            </body>
            </html>
            "#
            .to_string(),
        )
    } else {
        Html(
            r#"
            <!DOCTYPE html>
            <html>
            <head><title>Dvaar Login</title></head>
            <body style="font-family: sans-serif; text-align: center; padding: 50px;">
                <h1>Login Error</h1>
                <p>No token received.</p>
                <p>You can close this window.</p>
            </body>
            </html>
            "#
            .to_string(),
        )
    }
}

// URL encoding helper
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut encoded = String::new();
        for c in s.chars() {
            match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                    encoded.push(c);
                }
                _ => {
                    for byte in c.to_string().as_bytes() {
                        encoded.push_str(&format!("%{:02X}", byte));
                    }
                }
            }
        }
        encoded
    }
}
