//! `http-client-pro-web` — production binary for the axum web backend.
//!
//! Reads configuration from env vars:
//! - `HTTP_WEB_ADDR` — bind address (default `0.0.0.0:8080`)
//! - `HTTP_WEB_TOKEN` — optional bearer token for auth; if unset, no auth
//!
//! Health check: `GET /healthz` (no auth required).

use std::process::ExitCode;

use http_web::{Server, ServerBuilder};

#[tokio::main]
async fn main() -> ExitCode {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .try_init();

    let addr = std::env::var("HTTP_WEB_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let mut builder = ServerBuilder::new();
    if let Ok(token) = std::env::var("HTTP_WEB_TOKEN") {
        if !token.is_empty() {
            tracing::info!("auth enabled (token from HTTP_WEB_TOKEN)");
            builder = builder.with_token(token);
        }
    }
    let server: Server = builder.build();
    match server.serve_addr(&addr).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("http-web error: {e}");
            ExitCode::FAILURE
        }
    }
}
