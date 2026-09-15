//! Dispatcher integration tests — spec chapter 4.3 (multipart dispatch)
//! and TDD P2-5 (httpmock integration scenarios).
//!
//! These tests spin up a real `httpmock` server in-process and dispatch
//! parsed `.http` requests against it. They exercise the full pipeline:
//! parse → prepare (with file refs resolved) → reqwest dispatch → capture.

use http_core::dispatch::{
    is_binary_response, response_to_wire, save_binary_response, suggested_file_name,
    write_output_redirect, DispatchResponse, Dispatcher, SendOptions,
};
use http_core::env::Environment;
use http_core::model::OutputRedirect;
use http_core::parser::parse_file;
use httpmock::{Method, MockServer};
use std::io::Write;
use std::time::Duration;

fn first_req(src: &str) -> http_core::model::Request {
    parse_file(src)
        .unwrap()
        .requests
        .into_iter()
        .next()
        .unwrap()
}

fn env_of(pairs: &[(&str, &str)]) -> Environment {
    let mut e = Environment::new();
    for (k, v) in pairs {
        e.set(*k, *v);
    }
    e
}

fn tmp_fixture(name: &str, content: &[u8]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("http_core_dispatch_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(content).unwrap();
    path
}

// --- P2-5: GET 200 → status/headers/body captured ---

#[tokio::test]
async fn get_200_returns_status_headers_body() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(Method::GET).path("/api/get");
        then.status(200)
            .header("Content-Type", "text/plain")
            .body("hello");
    });

    let req = first_req(&format!("GET {}\n", server.url("/api/get")));
    let disp = Dispatcher::new();
    let res = disp
        .send(&req, &Environment::new(), None)
        .await
        .expect("dispatch ok");

    assert_eq!(res.status, 200);
    assert_eq!(res.header("Content-Type"), Some("text/plain"));
    assert_eq!(String::from_utf8_lossy(&res.body), "hello");
    mock.assert();
}

// --- P2-5: POST JSON body passthrough ---

#[tokio::test]
async fn post_json_body_passthrough() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(Method::POST)
            .path("/api/add")
            .header("Content-Type", "application/json")
            .body("{\"k\":\"v\"}");
        then.status(201).body("created");
    });

    let src = "\
POST <URL>
Content-Type: application/json

{\"k\":\"v\"}
";
    let src = src.replace("<URL>", &server.url("/api/add"));
    let req = first_req(&src);
    let disp = Dispatcher::new();
    let res = disp
        .send(&req, &Environment::new(), None)
        .await
        .expect("dispatch ok");

    assert_eq!(res.status, 201);
    mock.assert();
}

// --- P2-5: chunked response captured ---

#[tokio::test]
async fn chunked_response_captured() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(Method::GET).path("/stream");
        then.status(200)
            .header("Transfer-Encoding", "chunked")
            .body("chunk-1chunk-2chunk-3");
    });

    let req = first_req(&format!("GET {}\n", server.url("/stream")));
    let disp = Dispatcher::new();
    let res = disp
        .send(&req, &Environment::new(), None)
        .await
        .expect("dispatch ok");
    assert_eq!(res.status, 200);
    assert_eq!(String::from_utf8_lossy(&res.body), "chunk-1chunk-2chunk-3");
    let _ = mock; // silence unused warning if assertion ordering changes
}

// --- P2-5: connect error → diagnostic ---

#[tokio::test]
async fn connect_error_returns_diagnostic() {
    // Use a port that's almost certainly not listening (ephemeral reserved
    // range). 1 is reserved and will not bind.
    let req = first_req("GET http://127.0.0.1:1/api\n");
    let disp = Dispatcher::new();
    let res = disp.send(&req, &Environment::new(), None).await;
    assert!(res.is_err(), "expected an error for unreachable host");
    let err = res.unwrap_err();
    assert_eq!(err.kind, http_core::error::ErrorKind::Execute);
    assert!(
        err.message.to_lowercase().contains("connection")
            || err.message.to_lowercase().contains("connect")
    );
}

// --- P2-5: elapsed recorded ---

#[tokio::test]
async fn elapsed_recorded() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(Method::GET).path("/slow");
        then.status(200).delay(Duration::from_millis(50)).body("ok");
    });

    let req = first_req(&format!("GET {}\n", server.url("/slow")));
    let disp = Dispatcher::new();
    let res = disp
        .send(&req, &Environment::new(), None)
        .await
        .expect("dispatch ok");
    assert!(
        res.elapsed >= Duration::from_millis(20),
        "elapsed too small: {:?}",
        res.elapsed
    );
}

// --- P2-1 + P2-4: env var substitution + URL/path built into dispatch ---

#[tokio::test]
async fn dispatch_substitutes_env_vars_in_url() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(Method::GET).path("/api/items/42");
        then.status(200).body("ok");
    });

    let req = first_req(&format!("GET {}/api/items/{{{{id}}}}\n", server.base_url()));
    let env = env_of(&[("id", "42")]);
    let disp = Dispatcher::new();
    let res = disp.send(&req, &env, None).await.expect("dispatch ok");
    assert_eq!(res.status, 200);
    mock.assert();
}

// --- P2-3 + P2-5: inline body file ref resolved at dispatch ---

#[tokio::test]
async fn dispatch_inline_body_file_ref_reads_file_and_sends() {
    // spec 4.2.2: file body is NOT trimmed — the entire file content is sent.
    let fixture = tmp_fixture("inline_body.txt", b"\nmessage-body\n");
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(Method::POST)
            .path("/add")
            .body("\nmessage-body\n");
        then.status(200).body("ok");
    });

    let src = format!(
        "POST {url}/add\n\n< {file}\n",
        url = server.base_url(),
        file = fixture.display()
    );
    let req = first_req(&src);
    let disp = Dispatcher::new();
    let res = disp
        .send(&req, &Environment::new(), None)
        .await
        .expect("dispatch ok");
    assert_eq!(res.status, 200);
    mock.assert();
}

// --- P2-4: multipart string part + file part dispatched (spec 4.3) ---

#[tokio::test]
async fn dispatch_multipart_string_and_file_parts() {
    // spec 4.3 example: first part string ("Text"), second part file
    // (./input.txt). The dispatcher must send the file's actual content
    // inside the multipart body. httpmock 0.7 does not expose recorded
    // requests directly, so we assert on the request body via `when`
    // matchers (each `body_contains` constraint must be satisfied).
    let fixture = tmp_fixture("dispatch_input.txt", b"file-content");
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(Method::POST)
            .path("/api/upload")
            .body_contains("--abcd\r\n")
            .body_contains("Content-Disposition: form-data; name=\"text\"")
            .body_contains("\r\n\r\nText\r\n")
            .body_contains(
                "Content-Disposition: form-data; name=\"file_to_send\"; filename=\"input.txt\"",
            )
            .body_contains("file-content")
            .body_contains("--abcd--\r\n");
        then.status(200).body("ok");
    });

    let src = format!(
        "POST {url}/api/upload\n\
         Content-Type: multipart/form-data; boundary=abcd\n\
         \n\
         --abcd\n\
         Content-Disposition: form-data; name=\"text\"\n\
         \n\
         Text\n\
         --abcd\n\
         Content-Disposition: form-data; name=\"file_to_send\"; filename=\"input.txt\"\n\
         \n\
         < {file}\n\
         --abcd--\n",
        url = server.base_url(),
        file = fixture.display()
    );
    let req = first_req(&src);
    let disp = Dispatcher::new();
    let res = disp
        .send(&req, &Environment::new(), None)
        .await
        .expect("dispatch ok");
    assert_eq!(res.status, 200);
    mock.assert();
}

// --- P2-4: multipart boundary preserved as user-specified (spec 4.3) ---

#[tokio::test]
async fn dispatch_multipart_uses_user_specified_boundary() {
    // The Content-Type header carries `boundary=xyz` — the dispatcher must
    // NOT override it with an auto-generated boundary.
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(Method::POST)
            .path("/upload")
            .header("Content-Type", "multipart/form-data; boundary=xyz")
            .body_contains("--xyz\r\n")
            .body_contains("--xyz--\r\n");
        then.status(200);
    });

    let src = format!(
        "POST {url}/upload\n\
         Content-Type: multipart/form-data; boundary=xyz\n\
         \n\
         --xyz\n\
         Content-Disposition: form-data; name=\"f\"\n\
         \n\
         v\n\
         --xyz--\n",
        url = server.base_url()
    );
    let req = first_req(&src);
    let disp = Dispatcher::new();
    let res = disp
        .send(&req, &Environment::new(), None)
        .await
        .expect("dispatch ok");
    assert_eq!(res.status, 200);
    mock.assert();
}

// --- P2-1 + P2-5: header env var substitution in real dispatch ---

#[tokio::test]
async fn dispatch_substitutes_env_vars_in_headers() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(Method::GET)
            .path("/secret")
            .header("Authorization", "Bearer xyz");
        then.status(200).body("ok");
    });

    let src = format!(
        "GET {url}/secret\nAuthorization: Bearer {{{{token}}}}\n",
        url = server.base_url()
    );
    let req = first_req(&src);
    let env = env_of(&[("token", "xyz")]);
    let disp = Dispatcher::new();
    let res = disp.send(&req, &env, None).await.expect("dispatch ok");
    assert_eq!(res.status, 200);
    mock.assert();
}

// --- P2-5: response URL captured (redirects followed by default, plan §7.6) ---

#[tokio::test]
async fn dispatch_captures_response_url() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(Method::GET).path("/api");
        then.status(200);
    });

    let req = first_req(&format!("GET {}\n", server.url("/api")));
    let disp = Dispatcher::new();
    let res: DispatchResponse = disp
        .send(&req, &Environment::new(), None)
        .await
        .expect("dispatch ok");
    assert_eq!(res.url, server.url("/api"));
}

// --- P2-6: response ref `<> path` persists response body ---

#[tokio::test]
async fn dispatch_persists_response_to_response_ref_path() {
    // spec 3.2.5 + plan §5.3 step 7: after dispatch, the response body is
    // written to the file referenced by `<> path`. Existing files are
    // overwritten so the ref always holds the latest snapshot.
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(Method::GET).path("/api");
        then.status(200).body("response-body");
    });

    let fixture = tmp_fixture("response_ref_output.json", b"old-content");
    // Pre-existing content is overwritten.
    assert_eq!(std::fs::read(&fixture).unwrap(), b"old-content");

    let src = format!("GET {}\n\n<> {}\n", server.url("/api"), fixture.display());
    let req = first_req(&src);
    assert!(req.response_ref.is_some(), "parser captured response_ref");
    let disp = Dispatcher::new();
    let res = disp
        .send_and_persist(&req, &Environment::new(), None)
        .await
        .expect("dispatch ok");
    assert_eq!(res.status, 200);

    let written = std::fs::read(&fixture).unwrap();
    assert_eq!(
        &written, b"response-body",
        "response body persisted to ref path"
    );
}

#[tokio::test]
async fn dispatch_without_response_ref_does_not_write() {
    // No `<> ref` → send_and_persist is equivalent to send; no stray file writes.
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(Method::GET).path("/api");
        then.status(200).body("ok");
    });

    let req = first_req(&format!("GET {}\n", server.url("/api")));
    assert!(req.response_ref.is_none());
    let disp = Dispatcher::new();
    let res = disp
        .send_and_persist(&req, &Environment::new(), None)
        .await
        .expect("dispatch ok");
    assert_eq!(res.status, 200);
}

// --- P7-9: SendOptions derived from doc tags / HTTP version (plan §7.6) ---

#[test]
fn send_options_defaults_follow_redirects_and_cookie_jar() {
    let req = first_req("GET https://x.test/api\n");
    let opts = SendOptions::from_request(&req);
    assert!(opts.follow_redirects);
    assert!(opts.cookie_jar);
    assert_eq!(opts.timeout, None);
    assert_eq!(opts.connect_timeout, None);
    assert!(!opts.http2);
}

#[test]
fn send_options_from_tags_and_http2_version() {
    let req = first_req(
        "# @no-redirect\n\
         # @no-cookie-jar\n\
         # @timeout 1500\n\
         # @connection-timeout 400\n\
         GET https://x.test/api HTTP/2\n",
    );
    let opts = SendOptions::from_request(&req);
    assert!(!opts.follow_redirects);
    assert!(!opts.cookie_jar);
    assert_eq!(opts.timeout, Some(Duration::from_millis(1500)));
    assert_eq!(opts.connect_timeout, Some(Duration::from_millis(400)));
    assert!(opts.http2);
}

#[tokio::test]
async fn redirect_followed_by_default_but_not_with_tag() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(Method::GET).path("/start");
        then.status(302).header("Location", "/end");
    });
    server.mock(|when, then| {
        when.method(Method::GET).path("/end");
        then.status(200).body("final");
    });

    let disp = Dispatcher::new();
    // Default: follow → final body and URL.
    let req = first_req(&format!("GET {}\n", server.url("/start")));
    let res = disp.send_with(&req, &Environment::new(), None, &SendOptions::default()).await.unwrap();
    assert_eq!(res.status, 200);
    assert_eq!(res.url, server.url("/end"));

    // @no-redirect → the 302 is surfaced as-is.
    let req = first_req(&format!("# @no-redirect\nGET {}\n", server.url("/start")));
    let opts = SendOptions::from_request(&req);
    let res = disp.send_with(&req, &Environment::new(), None, &opts).await.unwrap();
    assert_eq!(res.status, 302);
    assert_eq!(res.header("Location"), Some("/end"));
}

#[tokio::test]
async fn cookies_shared_across_requests_in_jar() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(Method::GET).path("/set");
        then.status(200).header("Set-Cookie", "sid=abc123; Path=/");
    });
    // Only matches when the stored cookie is replayed.
    let check = server.mock(|when, then| {
        when.method(Method::GET)
            .path("/check")
            .header("Cookie", "sid=abc123");
        then.status(200).body("ok");
    });

    let disp = Dispatcher::new();
    let set = first_req(&format!("GET {}\n", server.url("/set")));
    disp.send_with(&set, &Environment::new(), None, &SendOptions::default())
        .await
        .unwrap();
    let get = first_req(&format!("GET {}\n", server.url("/check")));
    let res = disp
        .send_with(&get, &Environment::new(), None, &SendOptions::default())
        .await
        .unwrap();
    assert_eq!(res.status, 200, "cookie replayed from the shared jar");
    check.assert_hits(1);
}

#[tokio::test]
async fn no_cookie_jar_never_stores_or_replays_cookies() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(Method::GET).path("/set");
        then.status(200).header("Set-Cookie", "sid=abc123; Path=/");
    });
    let check = server.mock(|when, then| {
        when.method(Method::GET)
            .path("/check")
            .header_exists("Cookie");
        then.status(200);
    });

    let disp = Dispatcher::new();
    let set = first_req(&format!("# @no-cookie-jar\nGET {}\n", server.url("/set")));
    let opts = SendOptions::from_request(&set);
    assert!(!opts.cookie_jar);
    disp.send_with(&set, &Environment::new(), None, &opts).await.unwrap();

    let get = first_req(&format!("# @no-cookie-jar\nGET {}\n", server.url("/check")));
    let opts = SendOptions::from_request(&get);
    let res = disp.send_with(&get, &Environment::new(), None, &opts).await.unwrap();
    // No mock matches a cookie-less /check → httpmock answers 404.
    assert_eq!(res.status, 404);
    check.assert_hits(0);
}

// --- P7-10: `>> file` / `>>! file` output redirect persistence (plan §7.6 G14) ---

fn resp_of(body: &[u8]) -> DispatchResponse {
    DispatchResponse {
        status: 200,
        headers: vec![],
        body: body.to_vec(),
        elapsed: Duration::ZERO,
        url: "https://x.test/api".into(),
        http_version: "HTTP/1.1".into(),
    }
}

#[tokio::test]
async fn output_redirect_force_overwrites_existing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let redirect = OutputRedirect { path: "out.json".into(), force: true };
    // Pre-existing file is overwritten.
    std::fs::write(tmp.path().join("out.json"), b"old").unwrap();
    let written = write_output_redirect(&resp_of(b"fresh"), &redirect, &Environment::new(), Some(tmp.path())).unwrap();
    assert_eq!(written, tmp.path().join("out.json"));
    assert_eq!(std::fs::read(&written).unwrap(), b"fresh");
}

#[tokio::test]
async fn output_redirect_non_force_appends_numeric_suffix() {
    let tmp = tempfile::tempdir().unwrap();
    let redirect = OutputRedirect { path: "out.json".into(), force: false };
    let env = Environment::new();
    let resp = resp_of(b"one");
    // First write: out.json.
    let p1 = write_output_redirect(&resp, &redirect, &env, Some(tmp.path())).unwrap();
    assert_eq!(p1.file_name().unwrap(), "out.json");
    // Second: out-1.json, third: out-2.json.
    let p2 = write_output_redirect(&resp, &redirect, &env, Some(tmp.path())).unwrap();
    assert_eq!(p2.file_name().unwrap(), "out-1.json");
    let p3 = write_output_redirect(&resp, &redirect, &env, Some(tmp.path())).unwrap();
    assert_eq!(p3.file_name().unwrap(), "out-2.json");
    assert_eq!(std::fs::read(&p1).unwrap(), b"one");
}

#[tokio::test]
async fn output_redirect_substitutes_env_and_creates_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let mut env = Environment::new();
    env.set_project_root(tmp.path());
    let redirect = OutputRedirect { path: "{{$historyFolder}}/nested/r.json".into(), force: true };
    let written = write_output_redirect(&resp_of(b"body"), &redirect, &env, None).unwrap();
    assert_eq!(written, tmp.path().join(".http-history").join("nested").join("r.json"));
    assert_eq!(std::fs::read(&written).unwrap(), b"body");
}

// --- Binary responses: detection, download file name, persistence ---

/// Response with explicit headers, mirroring a file download.
fn resp_with(headers: &[(&str, &str)], body: &[u8]) -> DispatchResponse {
    DispatchResponse {
        status: 200,
        headers: headers
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect(),
        body: body.to_vec(),
        elapsed: Duration::ZERO,
        url: "https://x.test/api/arch/business/import".into(),
        http_version: "HTTP/1.1".into(),
    }
}

const XLSX_MIME: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet;charset=utf-8";

#[test]
fn attachment_disposition_is_binary_and_names_the_file_from_rfc5987() {
    let res = resp_with(
        &[
            (
                "Content-disposition",
                "attachment;filename*=utf-8''%E9%94%99%E8%AF%AF%E4%BF%A1%E6%81%AF.xlsx",
            ),
            ("Content-Type", XLSX_MIME),
        ],
        b"PK\x03\x04binary",
    );
    assert!(is_binary_response(&res));
    assert_eq!(suggested_file_name(&res), "错误信息.xlsx");
}

#[test]
fn spreadsheet_content_type_is_binary_and_falls_back_to_the_url_name() {
    let res = resp_with(&[("Content-Type", XLSX_MIME)], b"PK\x03\x04");
    assert!(is_binary_response(&res));
    assert_eq!(suggested_file_name(&res), "import");

    // No usable name anywhere → MIME-derived extension.
    let mut nameless = resp_with(&[("Content-Type", XLSX_MIME)], b"PK\x03\x04");
    nameless.url = "https://x.test/".into();
    assert_eq!(suggested_file_name(&nameless), "response.xlsx");
}

#[test]
fn textual_content_types_are_not_binary() {
    for ct in [
        "application/json",
        "application/json; charset=utf-8",
        "text/plain",
        "text/html; charset=UTF-8",
        "application/problem+json",
        "application/xml",
    ] {
        let res = resp_with(&[("Content-Type", ct)], b"{}");
        assert!(!is_binary_response(&res), "{ct} should render as text");
    }
}

#[test]
fn missing_content_type_falls_back_to_body_sniffing() {
    let text = resp_with(&[], b"{\"ok\":true}\n");
    assert!(!is_binary_response(&text));

    let zipped = resp_with(&[], b"PK\x03\x04\x00\x00\x00\x00");
    assert!(is_binary_response(&zipped));

    // An empty body is never a file download.
    assert!(!is_binary_response(&resp_with(&[], b"")));
}

#[test]
fn server_supplied_file_name_cannot_escape_the_target_dir() {
    let res = resp_with(
        &[("Content-Disposition", "attachment; filename=\"../../../etc/passwd\"")],
        b"x",
    );
    assert_eq!(suggested_file_name(&res), "passwd");

    let mut dots = resp_with(&[("Content-Disposition", "attachment; filename=\"..\"")], b"x");
    dots.url = "https://x.test/".into(); // no usable URL segment either
    assert_eq!(suggested_file_name(&dots), "response.bin");
}

#[test]
fn save_binary_response_creates_the_dir_and_keeps_every_run() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join(".http-history");
    let res = resp_with(
        &[
            ("Content-Disposition", "attachment; filename=\"错误信息.xlsx\""),
            ("Content-Type", XLSX_MIME),
        ],
        b"PK\x03\x04sheet",
    );

    let first = save_binary_response(&res, &dir).unwrap();
    let second = save_binary_response(&res, &dir).unwrap();

    assert_eq!(first, dir.join("错误信息.xlsx"));
    assert_eq!(second, dir.join("错误信息-1.xlsx"));
    assert_eq!(std::fs::read(&first).unwrap(), b"PK\x03\x04sheet");
}

#[tokio::test]
async fn dispatched_binary_response_is_detected_end_to_end() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(Method::GET).path("/download");
        then.status(200)
            .header("Content-Type", XLSX_MIME)
            .header(
                "Content-disposition",
                "attachment;filename*=utf-8''%E6%8A%A5%E8%A1%A8.xlsx",
            )
            .body("PK\u{3}\u{4}bytes");
    });

    let req = first_req(&format!("GET {}\n", server.url("/download")));
    let res = Dispatcher::new()
        .send(&req, &Environment::new(), None)
        .await
        .unwrap();

    assert_eq!(res.http_version, "HTTP/1.1");
    assert!(is_binary_response(&res));
    assert_eq!(suggested_file_name(&res), "报表.xlsx");
}

#[test]
fn response_to_wire_inlines_text_bodies() {
    let res = resp_with(
        &[("Content-Type", "application/json")],
        b"{\"ok\":true}",
    );
    let wire = response_to_wire(&res, None);
    assert_eq!(wire["binary"], serde_json::json!(false));
    assert_eq!(wire["body"], serde_json::json!("{\"ok\":true}"));
    assert_eq!(wire["content_length"], serde_json::json!(11));
    assert_eq!(wire["http_version"], serde_json::json!("HTTP/1.1"));
    assert!(wire["saved_path"].is_null());
}

#[test]
fn response_to_wire_saves_binary_and_reports_the_deduped_name() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join(".http-history");
    let res = resp_with(
        &[
            ("Content-Disposition", "attachment; filename=\"错误信息.xlsx\""),
            ("Content-Type", XLSX_MIME),
        ],
        b"PK\x03\x04sheet",
    );

    // Pre-create the target so the second save is de-duplicated (`-5` style).
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("错误信息.xlsx"), b"x").unwrap();

    let wire = response_to_wire(&res, Some(&dir));
    assert_eq!(wire["binary"], serde_json::json!(true));
    assert_eq!(wire["body"], serde_json::json!("")); // never inline raw bytes
    assert_eq!(wire["file_name"], serde_json::json!("错误信息-1.xlsx"));
    assert_eq!(
        wire["saved_path"],
        serde_json::json!(dir.join("错误信息-1.xlsx").to_string_lossy())
    );
    assert!(dir.join("错误信息-1.xlsx").exists());
}
