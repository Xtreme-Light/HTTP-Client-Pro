//! `http-mcp` — MCP server exposing `.http` file operations to AI agents
//! (plan §7 / TDD P5-3).
//!
//! Tools:
//! - `list_requests` — parse a `.http` source and return summaries.
//! - `run_request` — execute a request (subject to permission tiers).
//!
//! Permission tiers (plan §7 "MCP 权限分层"):
//! - `ReadOnly` — list only, no execution.
//! - `SafeWrite` — GET / HEAD / OPTIONS.
//! - `HighRiskWrite` — any method.

pub mod permissions;
pub mod server;
pub mod tools;

pub use permissions::PermissionTier;
pub use server::{serve_stdio, HttpMcpServer};
pub use tools::{list_requests, run_request, select_request, RequestSummary, ToolError};
