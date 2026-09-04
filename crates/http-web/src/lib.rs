//! http-web — axum REST + SSE Web backend for HTTP Request in Editor format
//! (plan §7 — Web/Docker end). Wraps `http-core`'s [`Dispatcher`] behind
//! an axum router exposing:
//!
//! - `POST /execute` — execute a `.http` request body, return JSON
//! - `GET /sse/execute?src=<http-source>` — stream execution events via SSE
//! - `GET /openapi.json` — OpenAPI 3.1 spec (utoipa)
//! - `GET /docs/` — Swagger UI
//!
//! Optional Bearer token auth (`with_token`) gates every route when set.

pub mod error;
pub mod routes;
pub mod state;

pub use error::{ApiError, ApiResult};
pub use routes::router;
pub use state::{AppState, Server, ServerBuilder};
