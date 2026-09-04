//! Error → HTTP response mapping.
//!
//! Upstream dispatch failures (e.g. connection refused) surface as `400
//! Bad Request` carrying a JSON `{ "error": <msg> }` body. Per RFC 7231
//! they aren't the server's fault strictly speaking, but clients calling
//! `POST /execute` expect actionable diagnostics, not a 500 that triggers
//! retry policies. Real server-side faults (auth, panic) stay 5xx.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use thiserror::Error;

pub type ApiResult<T> = std::result::Result<T, ApiError>;

#[derive(Debug, Error)]
pub enum ApiError {
    /// Upstream dispatch / prepare failure (e.g. host unreachable,
    /// parse error in the supplied .http source). Maps to 400 so clients
    /// get a diagnostic JSON body instead of a 500.
    #[error("{0}")]
    Dispatch(#[from] http_core::error::CoreError),

    /// Auth missing or wrong.
    #[error("unauthorized")]
    Unauthorized,

    /// Generic bad request (malformed body, missing required param).
    #[error("{0}")]
    BadRequest(String),
}

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        Self::BadRequest(format!("invalid JSON: {e}"))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            Self::Dispatch(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}
