//! The four JetBrains HTTP Client examples under `example/` — plan §7.9,
//! TDD P7-16.
//!
//! Two levels of checking:
//!
//! * **parse level** — every file parses without diagnostics and keeps the
//!   structure the examples document (`### name`, doc tags, indented request
//!   targets, `HTTP/2`, `>>` / `>>!` redirects, multipart file parts,
//!   pre-request scripts);
//! * **run level** — each file is staged into a temp dir next to its fixtures,
//!   retargeted from `https://httpbin.org` to an in-process `httpmock` fake,
//!   and executed end to end through [`run_file_with`].
//!
//! `HTTP/2` is stripped for the offline runs: httpmock speaks HTTP/1.1 only and
//! `HTTP/2` over a plain `http://` URL means h2c prior knowledge. Real HTTP/2
//! negotiation is covered by `public_smoke_test.rs`.

#![cfg(feature = "js-handler")]

use http_core::dispatch::Dispatcher;
use http_core::env::{load_env_files, Environment, Layer, VarValue};
use http_core::model::{
    MessageBody, MessagePartBody, Request, RequestTarget, RequestsFile, ResponseHandler,
};
use http_core::parser::parse_file;
use http_core::runner::{run_file_with, FileReport, RunOptions};
use httpmock::prelude::HttpMockRequest;
use httpmock::{Method, Mock, MockServer};
use std::path::{Path, PathBuf};

/// Fixtures the examples reference by relative path. Staged next to every
/// retargeted `.http` file so `< ./request-form-data.json`, `import "./my-utils"`
/// and the env files resolve exactly as they do in the repository.
const FIXTURES: [&str; 4] = [
    "http-client.env.json",
    "http-client.private.env.json",
    "request-form-data.json",
    "my-utils.js",
];

/// `sha256("payload-to-sign")` — the deterministic digest `my-utils.js`'
/// `makeSignature()` returns, so the ESM-import request can be matched exactly.
const ESM_SIGNATURE: &str = "b85a1426199228402fd429e4187226932f92325e29d1b318cfc996161a2b0e55";

fn example_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example")
}

fn example_src(name: &str) -> String {
    std::fs::read_to_string(example_dir().join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"))
}

fn parse_example(name: &str) -> RequestsFile {
    let file = parse_file(&example_src(name)).unwrap_or_else(|e| panic!("parse {name}: {e}"));
    assert!(file.diagnostics.is_empty(), "{name}: {:?}", file.diagnostics);
    file
}

fn query_of(req: &Request) -> Option<&str> {
    match &req.line.target {
        RequestTarget::Absolute { query, .. } | RequestTarget::Origin { query, .. } => {
            query.as_deref()
        }
        RequestTarget::Asterisk => None,
    }
}

// --- staging + running -------------------------------------------------------

/// Point the example at the fake server and drop the `HTTP/2` marker (see the
/// module docs for why).
fn retarget(src: &str, base: &str) -> String {
    src.replace("https://httpbin.org", base)
        .replace(" HTTP/2", "")
}

fn stage(dir: &Path, name: &str, base: &str) -> PathBuf {
    for fixture in FIXTURES {
        std::fs::copy(example_dir().join(fixture), dir.join(fixture))
            .unwrap_or_else(|e| panic!("copy {fixture}: {e}"));
    }
    let path = dir.join(name);
    std::fs::write(&path, retarget(&example_src(name), base)).unwrap();
    path
}

/// The `dev` environment from the staged env files, with `host` overridden so
/// GET.http's `GET {{host}}/get` reaches the fake server too.
fn dev_env(dir: &Path, base: &str) -> Environment {
    let mut env = load_env_files(Some("dev"), None, dir).expect("dev env loads");
    env.set_value(Layer::Environment, "host", VarValue::Str(base.to_string()));
    env
}

async fn run_staged(dir: &Path, name: &str, server: &MockServer) -> FileReport {
    let base = server.base_url();
    let path = stage(dir, name, &base);
    let env = dev_env(dir, &base);
    run_file_with(&path, env, &RunOptions::default(), &Dispatcher::new())
        .await
        .unwrap_or_else(|e| panic!("run {name}: {e}"))
}

fn statuses(report: &FileReport) -> Vec<u16> {
    report
        .requests
        .iter()
        .flat_map(|r| r.iterations.iter().map(|i| i.status))
        .collect()
}

/// Everything worth seeing when an example run goes sideways.
fn context(report: &FileReport) -> String {
    let tests: Vec<_> = report
        .requests
        .iter()
        .flat_map(|r| {
            r.iterations.iter().flat_map(|i| {
                i.tests
                    .iter()
                    .map(|t| (t.name.clone(), t.passed, t.message.clone()))
            })
        })
        .collect();
    let diags: Vec<_> = report
        .requests
        .iter()
        .flat_map(|r| {
            r.iterations
                .iter()
                .flat_map(|i| i.diagnostics.iter().map(|d| d.message.clone()))
        })
        .collect();
    format!("tests={tests:?}\ndiagnostics={diags:?}")
}

// --- the fake httpbin --------------------------------------------------------

/// `# @no-cookie-jar` means no `Cookie` header reaches the wire, while the
/// other `/cookies` request sends one explicitly. httpmock has no "header
/// absent" matcher, so the jar-less variant is matched by a function.
fn without_cookie_header(req: &HttpMockRequest) -> bool {
    match &req.headers {
        Some(headers) => !headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("cookie")),
        None => true,
    }
}

/// Scripts.http's HMAC request: an `X-My-Signature` that is a hex digest but
/// *not* the fixed value the imported ESM helper produces. Matching on shape
/// keeps the test independent of the private-env secret.
fn hmac_signature_header(req: &HttpMockRequest) -> bool {
    match &req.headers {
        Some(headers) => headers.iter().any(|(k, v)| {
            k.eq_ignore_ascii_case("x-my-signature")
                && v.len() == 64
                && v.chars().all(|c| c.is_ascii_hexdigit())
                && v != ESM_SIGNATURE
        }),
        None => false,
    }
}

fn get_echo() -> serde_json::Value {
    serde_json::json!({
        "args": { "show_env": "1", "generated-in": "IntelliJ IDEA" },
        "headers": {
            "Accept": "application/json",
            "Host": "httpbin.org",
            "User-Agent": "http-client-pro"
        },
        "origin": "203.0.113.7",
        "url": "https://httpbin.org/get"
    })
}

/// The handful of httpbin endpoints the four examples touch. Every `/post`
/// matcher is mutually exclusive so a request can only ever land on its own
/// mock — an unmatched request would 404 and fail the status assertions.
struct FakeHttpbin<'s> {
    cookies_no_jar: Mock<'s>,
    cookies_from_header: Mock<'s>,
    redirect: Mock<'s>,
    post_json: Mock<'s>,
    post_form: Mock<'s>,
    post_multipart: Mock<'s>,
    post_dynamic: Mock<'s>,
    post_esm: Mock<'s>,
    client_echoes: Vec<Mock<'s>>,
}

fn fake_httpbin(server: &MockServer) -> FakeHttpbin<'_> {
    // `json_body` does not set Content-Type, but httpbin does — and the
    // handlers need it to see `response.body` as a parsed object.
    let json = |then: httpmock::Then, body: serde_json::Value| {
        then.status(200)
            .header("Content-Type", "application/json")
            .json_body(body);
    };

    server.mock(|when, then| {
        when.method(Method::GET).path("/ip");
        json(then, serde_json::json!({ "origin": "203.0.113.7" }));
    });
    server.mock(|when, then| {
        when.method(Method::GET).path("/get");
        json(then, get_echo());
    });
    server.mock(|when, then| {
        when.method(Method::GET).path("/anything");
        json(
            then,
            serde_json::json!({ "args": {}, "url": "https://httpbin.org/anything" }),
        );
    });
    server.mock(|when, then| {
        when.method(Method::GET).path("/status/200");
        then.status(200);
    });
    server.mock(|when, then| {
        when.method(Method::GET).path("/status/404");
        then.status(404);
    });

    let redirect = server.mock(|when, then| {
        when.method(Method::GET).path("/status/301");
        then.status(301).header("Location", "https://httpbin.org/get");
    });

    let cookies_no_jar = server.mock(|when, then| {
        when.method(Method::GET)
            .path("/cookies")
            .matches(without_cookie_header);
        json(then, serde_json::json!({ "cookies": {} }));
    });
    let cookies_from_header = server.mock(|when, then| {
        when.method(Method::GET)
            .path("/cookies")
            .header_exists("Cookie");
        json(
            then,
            serde_json::json!({
                "cookies": { "theme": "darcula", "last_searched_location": "IJburg" }
            }),
        );
    });

    // POST.http — one mock per body shape.
    let post_json = server.mock(|when, then| {
        when.method(Method::POST)
            .path("/post")
            .body_contains(r#""id": 999"#);
        json(
            then,
            serde_json::json!({ "json": { "id": 999, "value": "content" } }),
        );
    });
    let post_form = server.mock(|when, then| {
        when.method(Method::POST)
            .path("/post")
            .body_contains("id=999&value=content&fact=IntelliJ");
        json(
            then,
            serde_json::json!({ "form": { "id": "999", "value": "content" } }),
        );
    });
    // Also pins that `< ./request-form-data.json` inside the multipart part was
    // read from disk and inlined.
    let post_multipart = server.mock(|when, then| {
        when.method(Method::POST)
            .path("/post")
            .body_contains("element-name")
            .body_contains("Sample payload attached as a multipart file field");
        json(
            then,
            serde_json::json!({ "form": { "element-name": "Name" } }),
        );
    });
    let post_dynamic = server.mock(|when, then| {
        when.method(Method::POST)
            .path("/post")
            .body_contains(r#""price""#);
        json(then, serde_json::json!({ "json": { "value": "content" } }));
    });

    // RequestWithScripts.http — crypto / ESM / clearAll.
    server.mock(|when, then| {
        when.method(Method::POST)
            .path("/post")
            .header_exists("X-My-Hash");
        json(then, serde_json::json!({ "json": { "prop": "value" } }));
    });
    server.mock(|when, then| {
        when.method(Method::POST)
            .path("/post")
            .matches(hmac_signature_header);
        json(then, serde_json::json!({ "json": { "prop": "value" } }));
    });
    // Echoes the signature back under `headers`, which is what the imported
    // `findSignature(response.body)` looks for.
    let post_esm = server.mock(|when, then| {
        when.method(Method::POST)
            .path("/post")
            .header("X-My-Signature", ESM_SIGNATURE);
        json(
            then,
            serde_json::json!({ "headers": { "X-My-Signature": ESM_SIGNATURE } }),
        );
    });
    server.mock(|when, then| {
        when.method(Method::POST)
            .path("/post")
            .body_contains("my-temp-variable");
        json(then, serde_json::json!({ "json": {} }));
    });

    // RequestWithLoop.http — both loop requests send `clientId`, so one echo per
    // client serves all six iterations.
    let client_echoes = [(1, "George", "Franklin", 100), (2, "John", "Doe", 1500), (3, "Eduardo", "Rodriquez", 10)]
        .into_iter()
        .map(|(id, first, last, balance)| {
            server.mock(move |when, then| {
                when.method(Method::POST)
                    .path("/post")
                    .body_contains(format!(r#""clientId": {id}"#));
                json(
                    then,
                    serde_json::json!({
                        "json": {
                            "clientId": id,
                            "firstName": first,
                            "lastName": last,
                            "balance": balance
                        }
                    }),
                );
            })
        })
        .collect();

    FakeHttpbin {
        cookies_no_jar,
        cookies_from_header,
        redirect,
        post_json,
        post_form,
        post_multipart,
        post_dynamic,
        post_esm,
        client_echoes,
    }
}

// --- parse level -------------------------------------------------------------

#[test]
fn fixtures_the_examples_reference_are_present() {
    for fixture in FIXTURES {
        assert!(example_dir().join(fixture).exists(), "missing {fixture}");
    }
    for name in [
        "GET.http",
        "POST.http",
        "RequestWithLoop.http",
        "RequestWithScripts.http",
    ] {
        assert!(example_dir().join(name).exists(), "missing {name}");
    }
}

#[test]
fn every_example_parses_into_the_expected_request_count() {
    // A trailing bare `###` (POST.http) separates but carries no request.
    for (name, expected) in [
        ("GET.http", 12),
        ("POST.http", 4),
        ("RequestWithLoop.http", 2),
        ("RequestWithScripts.http", 9),
    ] {
        let file = parse_example(name);
        assert_eq!(file.requests.len(), expected, "{name} request count");
    }
}

#[test]
fn get_example_keeps_names_tags_continuations_and_redirects() {
    let r = parse_example("GET.http").requests;

    // `### <text>` becomes the request name.
    assert_eq!(r[0].name.as_deref(), Some("GET request with a header"));
    assert!(r[1].name.is_none(), "a bare `###` yields no name");

    // An indented line continues the request target.
    assert_eq!(
        query_of(&r[1]),
        Some("generated-in=IntelliJ IDEA"),
        "indented `?generated-in=…` folded into the target"
    );

    // Doc tags bind to the request below them.
    assert!(r[4].tags.contains(&http_core::model::DocTag::NoRedirect));
    assert!(r[5].tags.contains(&http_core::model::DocTag::NoCookieJar));
    assert!(r[6].tags.contains(&http_core::model::DocTag::NoAutoEncoding));
    assert!(r[0].tags.is_empty());

    // `//TIP …` between the request and its trailer is insignificant.
    assert_eq!(
        r[9].output_redirect.as_ref().map(|o| (&o.path, o.force)),
        Some((&"{{$historyFolder}}/my-response.json".to_string(), false))
    );
    assert_eq!(
        r[10].output_redirect.as_ref().map(|o| (&o.path, o.force)),
        Some((&"{{$historyFolder}}/my-forced-response.json".to_string(), true))
    );

    // A bare `HTTP/2` on the request line.
    assert_eq!(r[11].line.http_version.as_deref(), Some("HTTP/2"));
    assert!(r[0].line.http_version.is_none());

    // `{{host}}` / `{{show_env}}` stay unresolved at parse time.
    match &r[3].line.target {
        RequestTarget::Absolute { authority, .. } => assert!(authority.contains("{{host}}")),
        other => panic!("expected an absolute target, got {other:?}"),
    }
}

#[test]
fn post_example_keeps_the_multipart_file_part() {
    let r = parse_example("POST.http").requests;

    let MessageBody::Multipart { boundary, parts } = &r[2].body.clone().expect("body") else {
        panic!("expected a multipart body");
    };
    assert_eq!(boundary, "WebAppBoundary");
    assert_eq!(parts.len(), 2);
    assert!(matches!(parts[0].body, MessagePartBody::Inline { .. }));
    assert_eq!(
        parts[1].body,
        MessagePartBody::FileRef {
            path: "./request-form-data.json".to_string()
        }
    );
    assert!(parts[1]
        .headers
        .iter()
        .any(|h| h.value.contains(r#"filename="data.json""#)));

    // The x-www-form-urlencoded body keeps its readable `k = v &` layout until
    // execution formats it.
    let MessageBody::Inline { content } = r[1].body.clone().expect("body") else {
        panic!("expected an inline body");
    };
    assert!(content.contains("id = 999 &"), "{content}");
}

#[test]
fn script_and_loop_examples_carry_their_pre_and_post_scripts() {
    let looped = parse_example("RequestWithLoop.http").requests;
    assert!(matches!(
        looped[0].pre_request_script,
        Some(ResponseHandler::Inline { .. })
    ));
    assert!(looped[1].pre_request_script.is_none());
    assert!(looped.iter().all(|r| matches!(
        r.response_handler,
        Some(ResponseHandler::Inline { .. })
    )));

    let scripts = parse_example("RequestWithScripts.http").requests;
    assert_eq!(
        scripts
            .iter()
            .filter(|r| r.pre_request_script.is_some())
            .count(),
        4
    );
    assert_eq!(
        scripts.iter().filter(|r| r.response_handler.is_some()).count(),
        7
    );
    // `client.global.clearAll()` runs as a pre-request script.
    let ResponseHandler::Inline { script } = scripts[8].pre_request_script.clone().expect("pre")
    else {
        panic!("expected an inline pre-request script");
    };
    assert!(script.contains("clearAll"), "{script}");
}

// --- run level ---------------------------------------------------------------

#[tokio::test]
async fn get_example_runs_against_a_fake_httpbin() {
    let server = MockServer::start();
    let fake = fake_httpbin(&server);
    let tmp = tempfile::tempdir().unwrap();

    let report = run_staged(tmp.path(), "GET.http", &server).await;

    assert_eq!(report.requests.len(), 12, "{}", context(&report));
    assert_eq!(
        statuses(&report),
        // `/status/301` is requested with `# @no-redirect`, so it must not be
        // followed; everything else is a plain 200.
        vec![200, 200, 200, 200, 301, 200, 200, 200, 200, 200, 200, 200],
        "{}",
        context(&report)
    );
    assert!(report.ok(), "{}", context(&report));
    assert_eq!(report.tests_failed, 0);

    fake.cookies_no_jar.assert_hits(1);
    fake.cookies_from_header.assert_hits(1);
    fake.redirect.assert_hits(1);

    // `>>` / `>>!` land under `$historyFolder`, rooted at the run's base dir.
    let history = tmp.path().join(".http-history");
    let saved = std::fs::read_to_string(history.join("my-response.json"))
        .expect("my-response.json written");
    assert!(saved.contains("203.0.113.7"), "{saved}");
    assert!(history.join("my-forced-response.json").exists());
}

#[tokio::test]
async fn post_example_runs_against_a_fake_httpbin() {
    let server = MockServer::start();
    let fake = fake_httpbin(&server);
    let tmp = tempfile::tempdir().unwrap();

    let report = run_staged(tmp.path(), "POST.http", &server).await;

    assert_eq!(report.requests.len(), 4, "{}", context(&report));
    assert_eq!(statuses(&report), vec![200; 4], "{}", context(&report));
    assert!(report.ok(), "{}", context(&report));

    fake.post_json.assert_hits(1);
    // The `k = v &` layout reached the wire percent-encoded, in order.
    fake.post_form.assert_hits(1);
    // `< ./request-form-data.json` was resolved and inlined into the part.
    fake.post_multipart.assert_hits(1);
    // `{{$random.uuid}}` was auto-quoted, `{{$random.integer()}}` stayed bare.
    fake.post_dynamic.assert_hits(1);
}

#[tokio::test]
async fn loop_example_runs_three_iterations_per_request() {
    let server = MockServer::start();
    let fake = fake_httpbin(&server);
    let tmp = tempfile::tempdir().unwrap();

    let report = run_staged(tmp.path(), "RequestWithLoop.http", &server).await;

    assert_eq!(report.requests.len(), 2, "{}", context(&report));
    assert_eq!(statuses(&report), vec![200; 6], "{}", context(&report));
    assert_eq!(report.tests_passed, 6, "{}", context(&report));
    assert_eq!(report.tests_failed, 0, "{}", context(&report));
    assert!(report.ok(), "{}", context(&report));

    // The second request logs the clientId and firstName of every iteration.
    assert_eq!(report.logs.len(), 6, "{:?}", report.logs);
    assert!(report.logs.iter().any(|l| l.contains("Eduardo")), "{:?}", report.logs);

    // Request 1 comes from the pre-request script, request 2 from the env file;
    // both walk the same three clients.
    for echo in &fake.client_echoes {
        echo.assert_hits(2);
    }
}

#[tokio::test]
async fn scripts_example_passes_everything_except_the_intended_failure() {
    let server = MockServer::start();
    let fake = fake_httpbin(&server);
    let tmp = tempfile::tempdir().unwrap();

    let report = run_staged(tmp.path(), "RequestWithScripts.http", &server).await;

    assert_eq!(report.requests.len(), 9, "{}", context(&report));
    assert_eq!(
        statuses(&report),
        vec![200, 404, 200, 200, 200, 200, 200, 200, 200],
        "{}",
        context(&report)
    );

    // Request 2 asserts `status === 200` against a 404 on purpose: the run must
    // record the failure, keep going, and report `ok() == false`.
    assert_eq!(report.tests_passed, 6, "{}", context(&report));
    assert_eq!(report.tests_failed, 1, "{}", context(&report));
    assert!(!report.ok());

    let failed = &report.requests[1].iterations[0].tests;
    assert_eq!(failed.len(), 1);
    assert!(!failed[0].passed);
    assert!(
        failed[0]
            .message
            .as_deref()
            .is_some_and(|m| m.contains("not 200")),
        "{failed:?}"
    );

    // `import { makeSignature } from "./my-utils"` resolved against the run's
    // base dir and produced the fixed digest.
    fake.post_esm.assert_hits(1);
}
