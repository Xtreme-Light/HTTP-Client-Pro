//! `http-client-pro-mcp` — MCP server binary.
//!
//! Reads the permission tier from the `HTTP_MCP_TIER` env var (defaults to
//! `safe_write`) and serves MCP tools over stdio. Logs go to stderr so
//! stdout stays clean for the MCP JSON-RPC protocol.

use std::process::ExitCode;

use http_mcp::permissions::PermissionTier;

#[tokio::main]
async fn main() -> ExitCode {
    // Log to stderr — stdout is reserved for MCP protocol messages.
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .try_init();

    let tier = std::env::var("HTTP_MCP_TIER")
        .ok()
        .and_then(|s| PermissionTier::from_str_lossy(&s))
        .unwrap_or_default();
    tracing::info!("starting MCP server with tier={}", tier.as_str());

    match http_mcp::serve_stdio(tier).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mcp server error: {e}");
            ExitCode::FAILURE
        }
    }
}
