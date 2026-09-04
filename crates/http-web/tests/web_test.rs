//! http-web integration tests — TDD P5-1.
//!
//! Each test spins up a real `http-web` server bound to an ephemeral port,
//! plus (where relevant) an `httpmock` upstream server to dispatch against.
//! Tests cover the REST, SSE, OpenAPI, and auth layers end-to-end.

use http_web::Server;
use serde_json::Value;

/// Bind a server to an ephemeral port on localhost, returning the URL prefix.
async fn spawn(no_auth: bool) -> String {
    let server = if no_auth {
        Server::builder().build()
    } else {
        Server::builder()
            .with_token("secret-token".to_string())
            .build()
    };
    let addr = server.bind_ephemeral().await;
    format!("http://{addr}")
}

async fn spawn_no_auth() -> String {
    spawn(true).await
}

async fn spawn_auth() -> String {
    let server = Server::builder()
        .with_token("secret-token".to_string())
        .build();
    let addr = server.bind_ephemeral().await;
    format!("http://{addr}")
}

// --- P5-1: POST /execute returns DispatchResponse JSON ---

#[tokio::test]
async fn post_execute_returns_dispatch_response_json() {
    // Stand up a mock upstream the .http request will hit.
    let upstream = httpmock::MockServer::start();
    upstream.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/api/get");
        then.status(200)
            .header("Content-Type", "text/plain")
            .body("hello");
    });

    let url = spawn_no_auth().await;
    let body = format!("GET {}\n", upstream.url("/api/get"));
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{url}/execute"))
        .header("Content-Type", "text/plain")
        .body(body)
        .send()
        .await
        .expect("send");
    assert_eq!(res.status(), 200);
    let json: Value = res.json().await.expect("json");
    assert_eq!(json["status"], 200);
    assert_eq!(json["body"], "hello");
    assert_eq!(json["url"], upstream.url("/api/get"));
}

// --- P5-1: POST /execute captures HTTP errors as 4xx, not 500 ---

#[tokio::test]
async fn post_execute_returns_4xx_for_upstream_unreachable() {
    // Use an obviously-closed port (port 1 is reserved, will not bind).
    let url = spawn_no_auth().await;
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{url}/execute"))
        .header("Content-Type", "text/plain")
        .body("GET http://127.0.0.1:1/api\n")
        .send()
        .await
        .expect("send");
    assert!(
        res.status().is_client_error(),
        "expected 4xx for unreachable upstream, got {}",
        res.status()
    );
    let json: Value = res.json().await.expect("json");
    assert!(json["error"].as_str().unwrap().contains("connection"));
}

// --- P5-1: OpenAPI spec is served ---

#[tokio::test]
async fn openapi_spec_served() {
    let url = spawn_no_auth().await;
    let res = reqwest::Client::new()
        .get(format!("{url}/openapi.json"))
        .send()
        .await
        .expect("send");
    assert_eq!(res.status(), 200);
    let spec: Value = res.json().await.expect("json");
    assert_eq!(spec["openapi"], "3.1.0");
    let paths = spec["paths"].as_object().expect("paths is object");
    assert!(paths.contains_key("/execute"), "spec lists /execute");
}

// --- P5-1: auth — missing token returns 401 ---

#[tokio::test]
async fn auth_missing_token_returns_401() {
    let url = spawn_auth().await;
    let res = reqwest::Client::new()
        .post(format!("{url}/execute"))
        .header("Content-Type", "text/plain")
        .body("GET http://example.com/\n")
        .send()
        .await
        .expect("send");
    assert_eq!(res.status(), 401);
}

// --- P5-1: auth — wrong token returns 401 ---

#[tokio::test]
async fn auth_wrong_token_returns_401() {
    let url = spawn_auth().await;
    let res = reqwest::Client::new()
        .post(format!("{url}/execute"))
        .header("Content-Type", "text/plain")
        .header("Authorization", "Bearer wrong-token")
        .body("GET http://example.com/\n")
        .send()
        .await
        .expect("send");
    assert_eq!(res.status(), 401);
}

// --- P5-1: auth — correct token passes ---

#[tokio::test]
async fn auth_correct_token_passes() {
    let upstream = httpmock::MockServer::start();
    upstream.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/api");
        then.status(200).body("ok");
    });

    let url = spawn_auth().await;
    let body = format!("GET {}\n", upstream.url("/api"));
    let res = reqwest::Client::new()
        .post(format!("{url}/execute"))
        .header("Content-Type", "text/plain")
        .header("Authorization", "Bearer secret-token")
        .body(body)
        .send()
        .await
        .expect("send");
    assert_eq!(res.status(), 200);
    let json: Value = res.json().await.expect("json");
    assert_eq!(json["status"], 200);
}

// --- P5-1: GET /sse/execute streams events ---

#[tokio::test]
async fn sse_execute_streams_events() {
    let upstream = httpmock::MockServer::start();
    upstream.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/api");
        then.status(200).body("ok");
    });

    let url = spawn_no_auth().await;
    let body = format!("GET {}\n", upstream.url("/api"));
    let res = reqwest::Client::new()
        .get(format!("{url}/sse/execute"))
        .query(&[("src", body.as_str())])
        .send()
        .await
        .expect("send");
    assert_eq!(res.status(), 200);
    assert_eq!(
        res.headers().get("content-type").unwrap(),
        "text/event-stream"
    );

    let text = res.text().await.expect("text");
    // SSE events are `event: <name>\ndata: <json>\n\n`. We expect at least
    // a final "done" event carrying the captured DispatchResponse.
    assert!(
        text.contains("event: done"),
        "expected done event, got: {text}"
    );
    assert!(
        text.contains("\"status\":200"),
        "done event carries status, got: {text}"
    );
}

// --- P5-4: GET /healthz returns ok without auth ---

#[tokio::test]
async fn healthz_returns_ok_without_auth() {
    // Server configured WITH a token — healthz must still respond 200.
    let url = spawn_auth().await;
    let res = reqwest::Client::new()
        .get(format!("{url}/healthz"))
        .send()
        .await
        .expect("send");
    assert_eq!(res.status(), 200);
    let json: Value = res.json().await.expect("json");
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn healthz_openapi_lists_healthz() {
    let url = spawn_no_auth().await;
    let res = reqwest::Client::new()
        .get(format!("{url}/openapi.json"))
        .send()
        .await
        .expect("send");
    assert_eq!(res.status(), 200);
    let spec: Value = res.json().await.expect("json");
    let paths = spec["paths"].as_object().expect("paths is object");
    assert!(paths.contains_key("/healthz"), "spec lists /healthz");
}
