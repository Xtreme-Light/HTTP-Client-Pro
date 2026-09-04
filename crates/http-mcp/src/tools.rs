//! Tool handler implementations for the MCP server.
//!
//! Two tools (plan §7 / TDD P5-3):
//! - `list_requests` — parse a `.http` file and return a compact list of
//!   request names, methods, and targets. Always available (even under
//!   `ReadOnly`).
//! - `run_request` — execute a single request by name or index, subject to
//!   the configured [`PermissionTier`].
//!
//! The handlers are plain async functions returning JSON values, so they
//! can be unit-tested without the rmcp transport. The `server` module
//! wraps them in `#[tool]`-annotated methods.

use http_core::dispatch::{DispatchResponse, Dispatcher};
use http_core::env::Environment;
use http_core::model::Request;
use http_core::parser::parse_file;
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::permissions::PermissionTier;

/// Error surfaced by tool handlers; the MCP server converts this to an
/// MCP error response.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("read {0}: {1}")]
    Read(PathBuf, String),
    #[error("parse: {0}")]
    Parse(String),
    #[error("request not found: {0}")]
    RequestNotFound(String),
    #[error("permission denied: {method} is not allowed under {tier}")]
    Permission {
        method: String,
        tier: PermissionTier,
    },
    #[error("dispatch: {0}")]
    Dispatch(String),
}

/// A single request's summary, returned by `list_requests`.
#[derive(Debug, Clone, Serialize)]
pub struct RequestSummary {
    /// 1-based position in the file.
    pub index: usize,
    /// Name from the `### name` separator, if any.
    pub name: Option<String>,
    /// HTTP method (GET, POST, ...).
    pub method: String,
    /// Request target as a single display string (URL or path).
    pub target: String,
}

/// Parse a `.http` file and return a list of request summaries.
/// Used by the `list_requests` tool. Does NOT execute anything — safe under
/// `ReadOnly` tier.
pub fn list_requests(src: &str) -> Result<Vec<RequestSummary>, ToolError> {
    let file = parse_file(src).map_err(|e| ToolError::Parse(e.to_string()))?;
    let summaries = file
        .requests
        .iter()
        .enumerate()
        .map(|(i, r)| RequestSummary {
            index: i + 1,
            name: r.name.clone(),
            method: r.line.method.to_string(),
            target: target_display(&r.line.target),
        })
        .collect();
    Ok(summaries)
}

/// Execute a single request selected by name (matching `### name`) or
/// 1-based index. The `tier` controls which HTTP methods may be executed.
pub async fn run_request(
    src: &str,
    selection: Option<&str>,
    tier: PermissionTier,
    env: &Environment,
    base_dir: Option<&Path>,
) -> Result<DispatchResponse, ToolError> {
    let file = parse_file(src).map_err(|e| ToolError::Parse(e.to_string()))?;
    let req = select_request(&file.requests, selection)?;
    if !tier.allows(&req.line.method) {
        return Err(ToolError::Permission {
            method: req.line.method.to_string(),
            tier,
        });
    }
    let dispatcher = Dispatcher::new();
    dispatcher
        .send(req, env, base_dir)
        .await
        .map_err(|e| ToolError::Dispatch(e.to_string()))
}

/// Select a request by name (matching `### name`) or 1-based numeric index.
/// `None` selects the first request.
pub fn select_request<'a>(
    requests: &'a [Request],
    selection: Option<&str>,
) -> Result<&'a Request, ToolError> {
    match selection {
        None => requests
            .first()
            .ok_or_else(|| ToolError::RequestNotFound("(empty file)".to_string())),
        Some(s) => {
            if let Some(req) = requests.iter().find(|r| r.name.as_deref() == Some(s)) {
                return Ok(req);
            }
            if let Ok(idx) = s.parse::<usize>() {
                if idx >= 1 && idx <= requests.len() {
                    return Ok(&requests[idx - 1]);
                }
            }
            Err(ToolError::RequestNotFound(s.to_string()))
        }
    }
}

/// Render a [`RequestTarget`] as a single human-readable string for display.
fn target_display(t: &http_core::model::RequestTarget) -> String {
    use http_core::model::RequestTarget;
    match t {
        RequestTarget::Asterisk => "*".to_string(),
        RequestTarget::Origin {
            path,
            query,
            fragment,
        } => {
            let mut s = path.clone();
            if let Some(q) = query {
                s.push('?');
                s.push_str(q);
            }
            if let Some(f) = fragment {
                s.push('#');
                s.push_str(f);
            }
            s
        }
        RequestTarget::Absolute {
            scheme,
            authority,
            path,
            query,
            fragment,
        } => {
            let scheme = scheme.as_deref().unwrap_or("http");
            let mut s = format!("{scheme}://{authority}");
            if let Some(p) = path {
                s.push_str(p);
            }
            if let Some(q) = query {
                s.push('?');
                s.push_str(q);
            }
            if let Some(f) = fragment {
                s.push('#');
                s.push_str(f);
            }
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_core::env::Environment;
    use httpmock::MockServer;
    use tempfile::TempDir;

    #[test]
    fn list_requests_parses_multiple_requests() {
        let src = "GET /a\n\n### login\nPOST /auth\n\n### logout\nDELETE /auth\n";
        let list = list_requests(src).expect("parse");
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].index, 1);
        assert_eq!(list[0].method, "GET");
        assert_eq!(list[0].target, "/a");
        assert!(list[0].name.is_none());
        assert_eq!(list[1].name.as_deref(), Some("login"));
        assert_eq!(list[1].method, "POST");
        assert_eq!(list[2].name.as_deref(), Some("logout"));
    }

    #[test]
    fn list_requests_handles_absolute_url() {
        let src = "GET https://example.com/api/v1/users\n";
        let list = list_requests(src).expect("parse");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].target, "https://example.com/api/v1/users");
    }

    #[test]
    fn list_requests_empty_file_returns_empty() {
        let list = list_requests("# just a comment\n").expect("parse");
        assert!(list.is_empty());
    }

    #[test]
    fn select_by_name() {
        let src = "GET /a\n\n### login\nPOST /auth\n";
        let file = parse_file(src).unwrap();
        let req = select_request(&file.requests, Some("login")).expect("found");
        assert_eq!(req.line.method, http_core::model::Method::Post);
    }

    #[test]
    fn select_by_index() {
        let src = "GET /a\n\n### second\nPOST /b\n";
        let file = parse_file(src).unwrap();
        let req = select_request(&file.requests, Some("2")).expect("found");
        assert_eq!(req.line.method, http_core::model::Method::Post);
    }

    #[test]
    fn select_first_when_none() {
        let src = "GET /a\n\nPOST /b\n";
        let file = parse_file(src).unwrap();
        let req = select_request(&file.requests, None).expect("found");
        assert_eq!(req.line.method, http_core::model::Method::Get);
    }

    #[test]
    fn select_unknown_errors() {
        let src = "GET /a\n";
        let file = parse_file(src).unwrap();
        let err = select_request(&file.requests, Some("missing")).unwrap_err();
        assert!(matches!(err, ToolError::RequestNotFound(_)));
    }

    #[tokio::test]
    async fn run_request_get_allowed_under_safe_write() {
        let upstream = MockServer::start();
        let m = upstream.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/api/get");
            then.status(200).body("ok-mcp");
        });

        let src = format!("GET {}\n", upstream.url("/api/get"));
        let env = Environment::new();
        let res = run_request(&src, None, PermissionTier::SafeWrite, &env, None)
            .await
            .expect("exec");
        assert_eq!(res.status, 200);
        assert_eq!(String::from_utf8_lossy(&res.body), "ok-mcp");
        m.assert_hits(1);
    }

    #[tokio::test]
    async fn run_request_post_blocked_under_safe_write() {
        let upstream = MockServer::start();
        upstream.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/post");
            then.status(200).body("created");
        });

        let src = format!("POST {}\n", upstream.url("/api/post"));
        let env = Environment::new();
        let err = run_request(&src, None, PermissionTier::SafeWrite, &env, None)
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Permission { .. }));
    }

    #[tokio::test]
    async fn run_request_post_allowed_under_high_risk() {
        let upstream = MockServer::start();
        let m = upstream.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/api/post");
            then.status(201).body("created");
        });

        let src = format!("POST {}\n", upstream.url("/api/post"));
        let env = Environment::new();
        let res = run_request(&src, None, PermissionTier::HighRiskWrite, &env, None)
            .await
            .expect("exec");
        assert_eq!(res.status, 201);
        m.assert_hits(1);
    }

    #[tokio::test]
    async fn run_request_readonly_blocks_get() {
        let upstream = MockServer::start();
        upstream.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/api/get");
            then.status(200);
        });

        let src = format!("GET {}\n", upstream.url("/api/get"));
        let env = Environment::new();
        let err = run_request(&src, None, PermissionTier::ReadOnly, &env, None)
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Permission { .. }));
    }

    #[tokio::test]
    async fn run_request_with_env_substitution() {
        let upstream = MockServer::start();
        let m = upstream.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/api/v1/items");
            then.status(200).body("env-works");
        });

        let host = upstream.url("").trim_start_matches("http://").to_string();
        let src = "GET http://{{host}}/api/v1/items\n".to_string();
        let mut env = Environment::new();
        env.set("host", &host);
        let res = run_request(&src, None, PermissionTier::SafeWrite, &env, None)
            .await
            .expect("exec");
        assert_eq!(res.status, 200);
        m.assert_hits(1);
    }

    #[tokio::test]
    async fn run_request_by_name() {
        let upstream = MockServer::start();
        let login_mock = upstream.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/auth/login");
            then.status(200).body("tok");
        });
        upstream.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/never-called");
            then.status(404);
        });

        let src = format!(
            "GET {}\n\n### login\nPOST {}\n",
            upstream.url("/never-called"),
            upstream.url("/auth/login"),
        );
        let env = Environment::new();
        let res = run_request(
            &src,
            Some("login"),
            PermissionTier::HighRiskWrite,
            &env,
            None,
        )
        .await
        .expect("exec");
        assert_eq!(res.status, 200);
        login_mock.assert_hits(1);
    }

    #[test]
    fn run_request_with_file_ref_uses_cwd() {
        // Just verify base_dir is plumbed through — file contents are read
        // relative to it. A full file-ref test lives in http-core.
        let tmp = TempDir::new().unwrap();
        let src = "GET /api\n";
        let env = Environment::new();
        // This will fail to connect (no upstream) but should not error on
        // missing files — there are none in this request.
        let rt = tokio::runtime::Runtime::new().unwrap();
        let res = rt.block_on(run_request(
            src,
            None,
            PermissionTier::SafeWrite,
            &env,
            Some(tmp.path()),
        ));
        // Expect a dispatch error (no upstream), NOT a file/permission error.
        assert!(matches!(res, Err(ToolError::Dispatch(_))));
    }
}
