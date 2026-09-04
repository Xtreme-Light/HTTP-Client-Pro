//! Response handler script tests (spec 4.5).
//!
//! Phase 3 TDD coverage:
//! - P3-1 JS sandbox: rquickjs init, script exceptions captured, no throw
//! - P3-2 client API: client.global.set/get persistence, client.log, assert
//! - P3-3 response API: response.status/body/headers
//! - P3-4 assert failure → diagnostic (no abort)
//! - P3-5 file handler loading

#![cfg(feature = "js-handler")]

use http_core::dispatch::DispatchResponse;
use http_core::handler::HandlerRuntime;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

fn sample_response(status: u16, body: &[u8], content_type: Option<&str>) -> DispatchResponse {
    let mut headers = Vec::new();
    if let Some(ct) = content_type {
        headers.push(("Content-Type".to_string(), ct.to_string()));
    }
    DispatchResponse {
        status,
        headers,
        body: body.to_vec(),
        elapsed: Duration::default(),
        url: "http://example.com".to_string(),
    }
}

fn tmp_fixture(name: &str, content: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join("http_core_handler_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(content).unwrap();
    path
}

// --- P3-1: sandbox init + script exception → diagnostic ---

#[test]
fn handler_runs_script_without_throwing() {
    let mut rt = HandlerRuntime::new();
    let resp = sample_response(200, b"{}", Some("application/json"));
    rt.run("1 + 2;", &resp).expect("script runs without error");
}

#[test]
fn handler_script_exception_recorded_as_diagnostic_not_bubbled() {
    // The sandbox must NOT bubble JS exceptions to Rust; they are converted
    // to diagnostics so subsequent requests can still execute.
    let mut rt = HandlerRuntime::new();
    let resp = sample_response(200, b"", None);
    rt.run(r#"throw new Error("boom");"#, &resp)
        .expect("exception captured, not bubbled");
    assert_eq!(rt.diagnostics().len(), 1);
    assert!(rt.diagnostics()[0].message.contains("boom"));
    assert_eq!(
        rt.diagnostics()[0].kind,
        http_core::error::ErrorKind::Handler
    );
}

// --- P3-2: client.global.set / get persistence ---

#[test]
fn handler_client_global_set_stores_value() {
    let mut rt = HandlerRuntime::new();
    let resp = sample_response(200, b"", None);
    rt.run(r#"client.global.set("token", "abc");"#, &resp)
        .unwrap();
    assert_eq!(rt.globals().get("token").map(String::as_str), Some("abc"));
}

#[test]
fn handler_client_global_get_returns_set_value() {
    let mut rt = HandlerRuntime::new();
    let resp = sample_response(200, b"", None);
    rt.run(
        r#"
        client.global.set("k", "v");
        client.log(client.global.get("k"));
        "#,
        &resp,
    )
    .unwrap();
    assert_eq!(rt.logs(), &["v".to_string()]);
}

#[test]
fn handler_globals_persist_across_run_calls() {
    // spec 4.5: client.global persists across requests so an auth handler can
    // set a token consumed by a later request's env.
    let mut rt = HandlerRuntime::new();
    let resp = sample_response(200, b"", None);
    rt.run(r#"client.global.set("token", "abc");"#, &resp)
        .unwrap();
    rt.run(r#"client.log(client.global.get("token"));"#, &resp)
        .unwrap();
    assert_eq!(rt.logs(), &["abc".to_string()]);
}

// --- P3-2: client.log capture ---

#[test]
fn handler_client_log_captures_multiple_args() {
    let mut rt = HandlerRuntime::new();
    let resp = sample_response(200, b"", None);
    rt.run(r#"client.log("hello", "world", 42);"#, &resp)
        .unwrap();
    assert_eq!(rt.logs(), &["hello world 42".to_string()]);
}

// --- P3-2 + P3-4: assert ---

#[test]
fn handler_assert_pass_does_not_record_diagnostic() {
    let mut rt = HandlerRuntime::new();
    let resp = sample_response(200, b"", None);
    rt.run(r#"assert(true, "should pass");"#, &resp).unwrap();
    assert!(
        rt.diagnostics().is_empty(),
        "passing assert should not record diagnostic"
    );
}

#[test]
fn handler_assert_fail_records_diagnostic_continues_execution() {
    // Assert failure must NOT throw — execution continues so the script can
    // record multiple failures in one pass.
    let mut rt = HandlerRuntime::new();
    let resp = sample_response(200, b"", None);
    rt.run(
        r#"
        assert(false, "first failure");
        assert(false, "second failure");
        "#,
        &resp,
    )
    .unwrap();
    assert_eq!(rt.diagnostics().len(), 2);
    assert!(rt.diagnostics()[0].message.contains("first failure"));
    assert!(rt.diagnostics()[1].message.contains("second failure"));
}

// --- P3-3: response object ---

#[test]
fn handler_response_status_exposed_as_number() {
    let mut rt = HandlerRuntime::new();
    let resp = sample_response(201, b"", None);
    rt.run(r#"client.log("status:", response.status);"#, &resp)
        .unwrap();
    assert_eq!(rt.logs(), &["status: 201".to_string()]);
}

#[test]
fn handler_response_body_string_when_no_content_type() {
    let mut rt = HandlerRuntime::new();
    let resp = sample_response(200, b"plain text body", None);
    rt.run(r#"client.log(response.body);"#, &resp).unwrap();
    assert_eq!(rt.logs(), &["plain text body".to_string()]);
}

#[test]
fn handler_response_body_object_when_json() {
    let mut rt = HandlerRuntime::new();
    let resp = sample_response(200, b"{\"token\":\"xyz\"}", Some("application/json"));
    rt.run(
        r#"
        var t = response.body.token;
        client.global.set("token", t);
        "#,
        &resp,
    )
    .unwrap();
    assert_eq!(rt.globals().get("token").map(String::as_str), Some("xyz"));
}

#[test]
fn handler_response_headers_value_case_insensitive() {
    let mut rt = HandlerRuntime::new();
    let resp = sample_response(200, b"", Some("text/plain; charset=utf-8"));
    rt.run(
        r#"
        client.log(response.headers.value("content-type"));
        "#,
        &resp,
    )
    .unwrap();
    assert_eq!(rt.logs(), &["text/plain; charset=utf-8".to_string()]);
}

#[test]
fn handler_response_headers_value_undefined_for_missing() {
    let mut rt = HandlerRuntime::new();
    let resp = sample_response(200, b"", None);
    rt.run(
        r#"
        var v = response.headers.value("Authorization");
        client.log(v === undefined ? "missing" : "present");
        "#,
        &resp,
    )
    .unwrap();
    assert_eq!(rt.logs(), &["missing".to_string()]);
}

// --- P3-5: file handler ---

#[test]
fn handler_file_handler_loaded_and_executed() {
    let script = r#"
        client.global.set("fromFile", "yes");
        client.log("loaded from file");
    "#;
    let path = tmp_fixture("handler.js", script.as_bytes());
    let mut rt = HandlerRuntime::new();
    let resp = sample_response(200, b"", None);
    rt.run_file(path.to_str().unwrap(), &resp).unwrap();
    assert_eq!(
        rt.globals().get("fromFile").map(String::as_str),
        Some("yes")
    );
    assert_eq!(rt.logs(), &["loaded from file".to_string()]);
}

#[test]
fn handler_file_handler_missing_file_returns_error() {
    let mut rt = HandlerRuntime::new();
    let resp = sample_response(200, b"", None);
    let res = rt.run_file("/nonexistent/path/handler.js", &resp);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.kind, http_core::error::ErrorKind::Io);
}

// --- P3-1: script timeout terminates infinite loop ---

#[test]
fn handler_script_timeout_terminates_infinite_loop() {
    // A runaway script (`while(true)`) must be killed by the interrupt
    // handler instead of hanging the test runner. Use a 100ms timeout.
    use std::time::Duration;
    let mut rt = HandlerRuntime::new().with_timeout(Duration::from_millis(100));
    let resp = sample_response(200, b"", None);
    rt.run("while (true) { }", &resp)
        .expect("timeout doesn't bubble");
    // The interrupt raises an uncatchable exception, surfaced as a diagnostic.
    assert_eq!(
        rt.diagnostics().len(),
        1,
        "expected exactly one timeout diagnostic"
    );
    assert!(rt.diagnostics()[0].kind == http_core::error::ErrorKind::Handler);
}

// --- P3-6: integration — auth flow sets a token consumed by a later request ---

#[test]
fn handler_auth_flow_token_consumed_by_subsequent_request() {
    // spec 4.5 + plan §5.4 / §5.5: the first response handler stashes a
    // token via `client.global.set`; the runtime exposes those globals so
    // the caller can merge them into the environment for subsequent
    // requests, which substitute `{{token}}` references.
    let mut rt = HandlerRuntime::new();

    // 1. Auth request returns a JSON token; handler stores it.
    let auth_resp = sample_response(200, br#"{"token":"xyz-123"}"#, Some("application/json"));
    rt.run(
        r#"
        client.global.set("token", response.body.token);
        "#,
        &auth_resp,
    )
    .unwrap();
    assert_eq!(
        rt.globals().get("token").map(String::as_str),
        Some("xyz-123")
    );

    // 2. The caller merges the runtime's globals into an environment; a
    //    later request substitutes `{{token}}` from that env.
    let mut env = http_core::env::Environment::new();
    for (k, v) in rt.globals() {
        env.set(k, v);
    }
    let substituted = http_core::env::substitute("Authorization: Bearer {{token}}", &env);
    assert_eq!(substituted, "Authorization: Bearer xyz-123");
}
