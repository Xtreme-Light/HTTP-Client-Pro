//! Public smoke tests — plan §7.10, TDD P7-17.
//!
//! These talk to the real `https://httpbin.org`, so every test is `#[ignore]`d.
//! Run them explicitly:
//!
//! ```text
//! cargo test -p http-core --features js-handler --test public_smoke_test -- \
//!     --ignored --test-threads 1
//! ```
//!
//! `--test-threads 1` matters: httpbin rate-limits, and the four example files
//! fire ~30 requests between them.
//!
//! What these cover that the offline `example_files_test.rs` cannot:
//! * real TLS + `HTTP/2` negotiation (httpmock is HTTP/1.1 only);
//! * redirect / cookie-jar / auto-encoding behaviour against a live server;
//! * that the examples still pass end to end against the host they name.

#![cfg(feature = "js-handler")]

use http_core::dispatch::Dispatcher;
use http_core::env::{load_env_files, Environment};
use http_core::parser::parse_file;
use http_core::runner::{run_file_with, run_parsed, FileReport, RunOptions};
use std::path::{Path, PathBuf};

const EXAMPLES: [&str; 4] = [
    "GET.http",
    "POST.http",
    "RequestWithLoop.http",
    "RequestWithScripts.http",
];

const FIXTURES: [&str; 4] = [
    "http-client.env.json",
    "http-client.private.env.json",
    "request-form-data.json",
    "my-utils.js",
];

fn example_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example")
}

/// Copy the examples and their fixtures **verbatim** into `dir`. Nothing is
/// retargeted — the point is to hit the real host — but running from a temp dir
/// keeps `>>` redirects and `$historyFolder` out of the repository.
fn stage_all(dir: &Path) {
    for name in EXAMPLES.iter().chain(FIXTURES.iter()) {
        std::fs::copy(example_dir().join(name), dir.join(name))
            .unwrap_or_else(|e| panic!("copy {name}: {e}"));
    }
}

fn dev_env(dir: &Path) -> Environment {
    load_env_files(Some("dev"), None, dir).expect("dev env loads")
}

async fn run_example(dir: &Path, name: &str) -> FileReport {
    run_file_with(
        &dir.join(name),
        dev_env(dir),
        &RunOptions::default(),
        &Dispatcher::new(),
    )
    .await
    .unwrap_or_else(|e| panic!("run {name}: {e}"))
}

async fn run_src(dir: &Path, src: &str) -> FileReport {
    let file = parse_file(src).expect("smoke source parses");
    run_parsed(&file, Environment::new(), dir, None, &Dispatcher::new())
        .await
        .expect("smoke run completes")
}

fn statuses(report: &FileReport) -> Vec<u16> {
    report
        .requests
        .iter()
        .flat_map(|r| r.iterations.iter().map(|i| i.status))
        .collect()
}

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

// --- endpoint level ----------------------------------------------------------

#[tokio::test]
#[ignore = "needs network: hits https://httpbin.org"]
async fn smoke_http2_is_negotiated_over_tls() {
    let tmp = tempfile::tempdir().unwrap();
    // The one request the offline suite has to neuter: `HTTP/2` on an `https://`
    // URL is ALPN negotiation, which httpmock cannot do.
    let report = run_src(tmp.path(), "GET https://httpbin.org/get HTTP/2\n").await;
    assert_eq!(statuses(&report), vec![200], "{}", context(&report));
}

#[tokio::test]
#[ignore = "needs network: hits https://httpbin.org"]
async fn smoke_no_redirect_tag_keeps_the_3xx() {
    let tmp = tempfile::tempdir().unwrap();
    let report = run_src(
        tmp.path(),
        "# @no-redirect\nGET https://httpbin.org/status/301\n",
    )
    .await;
    assert_eq!(statuses(&report), vec![301], "{}", context(&report));
}

#[tokio::test]
#[ignore = "needs network: hits https://httpbin.org"]
async fn smoke_cookies_are_echoed_from_the_header() {
    let tmp = tempfile::tempdir().unwrap();
    let report = run_src(
        tmp.path(),
        "\
### cookies
GET https://httpbin.org/cookies
Cookie: theme=darcula

> {%
    client.test(\"cookie echoed\", () => {
        client.assert(response.body.cookies.theme === \"darcula\")
    })
%}
",
    )
    .await;
    assert_eq!(statuses(&report), vec![200], "{}", context(&report));
    assert_eq!(report.tests_passed, 1, "{}", context(&report));
    assert!(report.ok(), "{}", context(&report));
}

#[tokio::test]
#[ignore = "needs network: hits https://httpbin.org"]
async fn smoke_json_post_is_echoed() {
    let tmp = tempfile::tempdir().unwrap();
    let report = run_src(
        tmp.path(),
        "\
### post
POST https://httpbin.org/post
Content-Type: application/json

{
  \"id\": 999,
  \"value\": \"content\"
}

> {%
    client.test(\"json echoed\", () => {
        client.assert(response.body.json.id === 999)
    })
%}
",
    )
    .await;
    assert_eq!(statuses(&report), vec![200], "{}", context(&report));
    assert_eq!(report.tests_passed, 1, "{}", context(&report));
}

// --- the four examples, unmodified -------------------------------------------

#[tokio::test]
#[ignore = "needs network: hits https://httpbin.org"]
async fn smoke_get_example() {
    let tmp = tempfile::tempdir().unwrap();
    stage_all(tmp.path());

    let report = run_example(tmp.path(), "GET.http").await;

    assert_eq!(report.requests.len(), 12, "{}", context(&report));
    assert_eq!(
        statuses(&report),
        vec![200, 200, 200, 200, 301, 200, 200, 200, 200, 200, 200, 200],
        "{}",
        context(&report)
    );
    assert!(report.ok(), "{}", context(&report));
    assert!(tmp
        .path()
        .join(".http-history/my-response.json")
        .exists());
    assert!(tmp
        .path()
        .join(".http-history/my-forced-response.json")
        .exists());
}

#[tokio::test]
#[ignore = "needs network: hits https://httpbin.org"]
async fn smoke_post_example() {
    let tmp = tempfile::tempdir().unwrap();
    stage_all(tmp.path());

    let report = run_example(tmp.path(), "POST.http").await;

    assert_eq!(report.requests.len(), 4, "{}", context(&report));
    assert_eq!(statuses(&report), vec![200; 4], "{}", context(&report));
    assert!(report.ok(), "{}", context(&report));
}

#[tokio::test]
#[ignore = "needs network: hits https://httpbin.org"]
async fn smoke_loop_example() {
    let tmp = tempfile::tempdir().unwrap();
    stage_all(tmp.path());

    let report = run_example(tmp.path(), "RequestWithLoop.http").await;

    assert_eq!(report.requests.len(), 2, "{}", context(&report));
    assert_eq!(statuses(&report), vec![200; 6], "{}", context(&report));
    assert_eq!(report.tests_passed, 6, "{}", context(&report));
    assert_eq!(report.tests_failed, 0, "{}", context(&report));
    assert!(report.ok(), "{}", context(&report));
}

#[tokio::test]
#[ignore = "needs network: hits https://httpbin.org"]
async fn smoke_scripts_example() {
    let tmp = tempfile::tempdir().unwrap();
    stage_all(tmp.path());

    let report = run_example(tmp.path(), "RequestWithScripts.http").await;

    assert_eq!(report.requests.len(), 9, "{}", context(&report));
    assert_eq!(
        statuses(&report),
        vec![200, 404, 200, 200, 200, 200, 200, 200, 200],
        "{}",
        context(&report)
    );
    // Request 2 asserts `status === 200` against a 404 on purpose.
    assert_eq!(report.tests_passed, 6, "{}", context(&report));
    assert_eq!(report.tests_failed, 1, "{}", context(&report));
    assert!(!report.ok());
}
