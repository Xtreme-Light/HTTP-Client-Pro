//! rmcp server wiring: wraps the tool handlers in `#[tool]`-annotated
//! methods and runs them over stdio transport.
//!
//! The server holds a [`PermissionTier`] (configured at startup) and a
//! working directory for `< file` references. The `list_requests` tool is
//! always available; `run_request` is gated by the tier.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars;
use rmcp::tool;
use rmcp::tool_handler;
use rmcp::tool_router;
use rmcp::transport::stdio;
use rmcp::ServerHandler;
use rmcp::ServiceExt;

use http_core::env::Environment;

use crate::permissions::PermissionTier;
use crate::tools::{self, ToolError};

/// Parameters for the `list_requests` tool.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListRequestsParams {
    /// The `.http` source text to parse.
    #[serde(rename = "source")]
    pub source: String,
}

/// Parameters for the `run_request` tool.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RunRequestParams {
    /// The `.http` source text to parse.
    #[serde(rename = "source")]
    pub source: String,
    /// Optional request name (matching `### name`) or 1-based index. If
    /// omitted, the first request is used.
    #[serde(rename = "request", default)]
    pub request: Option<String>,
    /// Optional environment variables as `{"name":"value"}` pairs. These
    /// are applied for `{{var}}` substitution in the request.
    #[serde(rename = "env", default)]
    pub env: std::collections::BTreeMap<String, String>,
}

/// The MCP server. Holds configuration that applies to all tool calls.
#[derive(Clone)]
pub struct HttpMcpServer {
    tier: PermissionTier,
}

impl HttpMcpServer {
    pub fn new(tier: PermissionTier) -> Self {
        Self { tier }
    }
}

#[tool_router]
impl HttpMcpServer {
    #[tool(
        description = "List all HTTP requests defined in a .http file source. Returns each request's index, name, method, and target. Does not execute any request."
    )]
    fn list_requests(
        &self,
        Parameters(ListRequestsParams { source }): Parameters<ListRequestsParams>,
    ) -> Result<String, rmcp::model::ErrorData> {
        let list = tools::list_requests(&source).map_err(tool_err_to_mcp)?;
        Ok(serde_json::to_string_pretty(&list).unwrap_or_else(|e| format!("error: {e}")))
    }

    #[tool(
        description = "Execute a single HTTP request from a .http file source. The request is selected by name (matching the ### separator comment) or 1-based index. Environment variables for {{var}} substitution can be provided. Returns the response status, headers, body, and elapsed time."
    )]
    fn run_request(
        &self,
        Parameters(RunRequestParams {
            source,
            request,
            env,
        }): Parameters<RunRequestParams>,
    ) -> Result<String, rmcp::model::ErrorData> {
        // We can't easily make this async with the #[tool] macro's sync
        // signature. rmcp's tool handlers can be async — but the macro
        // expects a specific shape. For now, block on a runtime.
        // TODO: switch to async tool handler when rmcp macro supports it.
        let tier = self.tier;
        let mut env_obj = Environment::new();
        for (k, v) in env {
            env_obj.set(k, v);
        }
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| rmcp::model::ErrorData::internal_error(format!("runtime: {e}"), None))?;
        let res = rt
            .block_on(tools::run_request(
                &source,
                request.as_deref(),
                tier,
                &env_obj,
                None,
            ))
            .map_err(tool_err_to_mcp)?;
        // Format as a human-readable string (MCP text content).
        let body = String::from_utf8_lossy(&res.body);
        let header_lines: Vec<String> = res
            .headers
            .iter()
            .map(|(n, v)| format!("{n}: {v}"))
            .collect();
        Ok(format!(
            "Status: {}\nURL: {}\nElapsed: {} ms\nHeaders:\n{}\n\nBody:\n{}",
            res.status,
            res.url,
            res.elapsed.as_millis(),
            header_lines.join("\n"),
            body,
        ))
    }
}

#[tool_handler(
    name = "http-client-pro",
    version = "0.1.0",
    instructions = "HTTP Client Pro MCP server. Provides tools to list and execute HTTP requests defined in .http (HTTP Request in Editor) format files."
)]
impl ServerHandler for HttpMcpServer {}

/// Run the MCP server over stdio. Blocks until the client disconnects.
pub async fn serve_stdio(tier: PermissionTier) -> anyhow::Result<()> {
    let server = HttpMcpServer::new(tier);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

fn tool_err_to_mcp(e: ToolError) -> rmcp::model::ErrorData {
    rmcp::model::ErrorData::internal_error(e.to_string(), None)
}
