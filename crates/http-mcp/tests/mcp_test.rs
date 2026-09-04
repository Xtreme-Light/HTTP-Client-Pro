//! Integration tests for http-mcp — verify the tool handlers work
//! end-to-end against a mock HTTP server and respect permission tiers.
//!
//! These tests exercise the public `tools::` functions directly rather
//! than the rmcp transport (which would require a full JSON-RPC client).

use http_core::env::Environment;
use http_mcp::permissions::PermissionTier;
use http_mcp::tools::{list_requests, run_request, select_request, RequestSummary};
use httpmock::MockServer;

#[test]
fn list_requests_returns_summaries() {
    let src = "GET /a\n\n### login\nPOST /auth\n\n### get-user\nGET /users/me\n";
    let list: Vec<RequestSummary> = list_requests(src).expect("parse");
    assert_eq!(list.len(), 3);
    assert_eq!(list[0].index, 1);
    assert_eq!(list[0].method, "GET");
    assert!(list[0].name.is_none());
    assert_eq!(list[1].name.as_deref(), Some("login"));
    assert_eq!(list[1].method, "POST");
    assert_eq!(list[2].name.as_deref(), Some("get-user"));
}

#[test]
fn list_requests_handles_absolute_url() {
    let src = "GET https://api.example.com/v1/users?page=1\n";
    let list = list_requests(src).expect("parse");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].target, "https://api.example.com/v1/users?page=1");
}

#[tokio::test]
async fn run_request_get_under_safe_write() {
    let upstream = MockServer::start();
    let m = upstream.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/api/items");
        then.status(200).body("[]");
    });
    let src = format!("GET {}\n", upstream.url("/api/items"));
    let env = Environment::new();
    let res = run_request(&src, None, PermissionTier::SafeWrite, &env, None)
        .await
        .expect("exec");
    assert_eq!(res.status, 200);
    m.assert_hits(1);
}

#[tokio::test]
async fn run_request_post_blocked_under_safe_write() {
    let upstream = MockServer::start();
    upstream.mock(|when, then| {
        when.method(httpmock::Method::POST).path("/api/items");
        then.status(201);
    });
    let src = format!("POST {}\n", upstream.url("/api/items"));
    let env = Environment::new();
    let err = run_request(&src, None, PermissionTier::SafeWrite, &env, None)
        .await
        .unwrap_err();
    assert!(matches!(err, http_mcp::ToolError::Permission { .. }));
}

#[tokio::test]
async fn run_request_post_allowed_under_high_risk() {
    let upstream = MockServer::start();
    let m = upstream.mock(|when, then| {
        when.method(httpmock::Method::POST).path("/api/items");
        then.status(201).body("created");
    });
    let src = format!("POST {}\n", upstream.url("/api/items"));
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
        when.method(httpmock::Method::GET).path("/api/items");
        then.status(200);
    });
    let src = format!("GET {}\n", upstream.url("/api/items"));
    let env = Environment::new();
    let err = run_request(&src, None, PermissionTier::ReadOnly, &env, None)
        .await
        .unwrap_err();
    assert!(matches!(err, http_mcp::ToolError::Permission { .. }));
}

#[tokio::test]
async fn run_request_by_name_skips_unnamed() {
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

#[tokio::test]
async fn run_request_with_env_vars() {
    let upstream = MockServer::start();
    let m = upstream.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/api/v1/items");
        then.status(200);
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

#[test]
fn select_request_empty_file_errors() {
    let src = "# just a comment\n";
    // Parse returns empty requests — select from empty should error.
    let file = http_core::parser::parse_file(src).unwrap();
    let err = select_request(&file.requests, None).unwrap_err();
    assert!(matches!(err, http_mcp::ToolError::RequestNotFound(_)));
}

#[test]
fn permission_tier_default_is_safe_write() {
    assert_eq!(PermissionTier::default(), PermissionTier::SafeWrite);
}

#[test]
fn permission_tier_from_str_accepts_aliases() {
    assert_eq!(
        PermissionTier::from_str_lossy("read_only"),
        Some(PermissionTier::ReadOnly)
    );
    assert_eq!(
        PermissionTier::from_str_lossy("safe-write"),
        Some(PermissionTier::SafeWrite)
    );
    assert_eq!(
        PermissionTier::from_str_lossy("full"),
        Some(PermissionTier::HighRiskWrite)
    );
}
