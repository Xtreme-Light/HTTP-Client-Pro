//! Batch runner tests — plan §7.8 (G23) / TDD P7-14.
//!
//! Everything runs offline: requests are dispatched to an in-process
//! `httpmock` server, and files (`>>` redirects, handler refs) land in a
//! `tempfile::TempDir` used as the run's base directory.

#![cfg(feature = "js-handler")]

use http_core::dispatch::Dispatcher;
use http_core::env::Environment;
use http_core::parser::parse_file;
use http_core::runner::{run_file_with, run_parsed, FileReport, RunOptions};
use httpmock::{Method, MockServer};
use std::path::Path;

/// Parse `src` and run every request in it against whatever host it names.
async fn run(src: &str, env: Environment, base_dir: &Path) -> FileReport {
    run_with_filter(src, env, base_dir, None).await
}

async fn run_with_filter(
    src: &str,
    env: Environment,
    base_dir: &Path,
    filter: Option<&str>,
) -> FileReport {
    let file = parse_file(src).expect("example source parses");
    run_parsed(&file, env, base_dir, filter, &Dispatcher::new())
        .await
        .expect("run completes")
}

/// All tests in a report, flattened with the name of their request.
fn all_tests(report: &FileReport) -> Vec<(Option<String>, String, bool)> {
    report
        .requests
        .iter()
        .flat_map(|r| {
            r.iterations.iter().flat_map(move |it| {
                it.tests
                    .iter()
                    .map(move |t| (r.name.clone(), t.name.clone(), t.passed))
            })
        })
        .collect()
}

// --- P7-14: the whole file runs, in order ---

#[tokio::test]
async fn runs_every_request_in_file_in_order() {
    let server = MockServer::start();
    let a = server.mock(|when, then| {
        when.method(Method::GET).path("/a");
        then.status(200).body("A");
    });
    let b = server.mock(|when, then| {
        when.method(Method::GET).path("/b");
        then.status(201).body("B");
    });
    let tmp = tempfile::tempdir().unwrap();

    let src = "\
### first
GET <BASE>/a

### second
GET <BASE>/b
"
    .replace("<BASE>", &server.base_url());

    let report = run(&src, Environment::new(), tmp.path()).await;

    assert_eq!(report.requests.len(), 2);
    assert_eq!(report.requests[0].name.as_deref(), Some("first"));
    assert_eq!(report.requests[1].name.as_deref(), Some("second"));
    assert_eq!(report.requests[0].iterations[0].status, 200);
    assert_eq!(report.requests[1].iterations[0].status, 201);
    assert!(report.ok(), "no assertions ran, so nothing failed");
    a.assert_hits(1);
    b.assert_hits(1);
}

// --- P7-14: `client.global` survives across requests ---

#[tokio::test]
async fn globals_set_by_one_request_are_visible_to_the_next() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(Method::GET).path("/login");
        then.status(200).body("ok");
    });
    // Only matches when the global set by the first request is substituted.
    let second = server.mock(|when, then| {
        when.method(Method::GET)
            .path("/me")
            .header("Authorization", "Bearer abc123");
        then.status(200).body("profile");
    });
    let tmp = tempfile::tempdir().unwrap();

    let src = "\
### login
GET <BASE>/login

> {%
    client.global.set(\"tok\", \"abc123\")
%}

### me
GET <BASE>/me
Authorization: Bearer {{tok}}
"
    .replace("<BASE>", &server.base_url());

    let report = run(&src, Environment::new(), tmp.path()).await;

    assert_eq!(report.requests.len(), 2);
    assert_eq!(report.requests[1].iterations[0].status, 200);
    second.assert_hits(1);
}

// --- P7-14: a failing request neither hides nor blocks anything ---

#[tokio::test]
async fn failed_test_is_counted_and_later_requests_still_run() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(Method::GET).path("/missing");
        then.status(404).body("nope");
    });
    let later = server.mock(|when, then| {
        when.method(Method::GET).path("/ok");
        then.status(200).body("fine");
    });
    let tmp = tempfile::tempdir().unwrap();

    let src = "\
### expect failure
GET <BASE>/missing

> {%
    client.test(\"status is 200\", function () {
        client.assert(response.status === 200, \"Response status is not 200\");
    });
%}

### expect success
GET <BASE>/ok

> {%
    client.test(\"status is 200\", function () {
        client.assert(response.status === 200, \"Response status is not 200\");
    });
%}
"
    .replace("<BASE>", &server.base_url());

    let report = run(&src, Environment::new(), tmp.path()).await;

    assert_eq!(report.requests.len(), 2, "the failure did not stop the run");
    assert_eq!(report.tests_passed, 1);
    assert_eq!(report.tests_failed, 1);
    assert!(!report.ok());

    let tests = all_tests(&report);
    assert_eq!(tests.len(), 2);
    assert!(!tests[0].2, "first request's assertion failed");
    assert!(tests[1].2, "second request's assertion passed");
    let message = report.requests[0].iterations[0].tests[0]
        .message
        .clone()
        .unwrap_or_default();
    assert!(message.contains("not 200"), "assertion text kept: {message}");
    later.assert_hits(1);
}

// --- P7-14: pre-request script runs before dispatch ---

#[tokio::test]
async fn pre_request_script_variables_reach_the_wire() {
    let server = MockServer::start();
    let hit = server.mock(|when, then| {
        when.method(Method::GET)
            .path("/pre")
            .header("X-Token", "pre-123");
        then.status(200).body("ok");
    });
    let tmp = tempfile::tempdir().unwrap();

    let src = "\
### with pre-request script
< {%
    request.variables.set(\"tok\", \"pre-123\")
%}

GET <BASE>/pre
X-Token: {{tok}}
"
    .replace("<BASE>", &server.base_url());

    let report = run(&src, Environment::new(), tmp.path()).await;

    assert_eq!(report.requests[0].iterations[0].status, 200);
    hit.assert_hits(1);
}

// --- P7-14 / P7-10: JSONPath loop drives one dispatch per match ---

const CLIENTS_SCRIPT: &str = "\
### loop over clients
< {%
  request.variables.set(\"clients\", [
    {\"id\": 1, \"firstName\": \"George\", \"lastName\": \"Franklin\", balance: 100},
    {\"id\": 2, \"firstName\": \"John\", \"lastName\": \"Doe\", balance: 1500},
    {\"id\": 3, \"firstName\": \"Eduardo\", \"lastName\": \"Rodriquez\", balance: 10}
  ])
%}

POST <BASE>/post
Content-Type: application/json

{
  \"clientId\": {{$.clients..id}},
  \"firstName\": \"{{$.clients..firstName}}\",
  \"balance\": \"{{$.clients..balance}}\"
}

> {%
  let current = request.variables.get(\"clients\")[request.iteration()]
  client.log(`iteration ${request.iteration()} -> ${current.lastName}`)
  client.test(`Account ${current.lastName} has balance ${current.balance}`, () => {
    let echoed = jsonPath(response.body, \"$.json.balance\")
    client.assert(echoed == current.balance, \"balance mismatch\")
    client.assert(request.templateValue(0) == current.id, \"templateValue(0) mismatch\")
  })
%}
";

/// A httpbin-style `/post` echo: one mock per expected `clientId`, each
/// returning the matching balance so the handler's assertion is meaningful.
/// The handles are returned so callers can assert each mock was hit.
fn mock_client_echoes(server: &MockServer) -> Vec<httpmock::Mock<'_>> {
    [(1, "George", 100), (2, "John", 1500), (3, "Eduardo", 10)]
        .into_iter()
        .map(|(id, first_name, balance)| {
            server.mock(move |when, then| {
                when.method(Method::POST)
                    .path("/post")
                    .body_contains(format!("\"clientId\": {id}"));
                // `json_body` does not set Content-Type; httpbin does, and the
                // handler needs it to see `response.body` as a parsed object.
                then.status(200)
                    .header("Content-Type", "application/json")
                    .json_body(serde_json::json!({
                        "json": { "clientId": id, "firstName": first_name, "balance": balance }
                    }));
            })
        })
        .collect()
}

#[tokio::test]
async fn loop_request_dispatches_once_per_jsonpath_match() {
    let server = MockServer::start();
    let echoes = mock_client_echoes(&server);
    let tmp = tempfile::tempdir().unwrap();

    let src = CLIENTS_SCRIPT.replace("<BASE>", &server.base_url());
    let report = run(&src, Environment::new(), tmp.path()).await;

    assert_eq!(report.requests.len(), 1);
    let iterations = &report.requests[0].iterations;
    assert_eq!(iterations.len(), 3, "one dispatch per client");
    assert_eq!(
        iterations.iter().map(|i| i.index).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert!(iterations.iter().all(|i| i.status == 200));
    assert_eq!(
        report.tests_passed,
        3,
        "tests: {tests:?}, messages: {msgs:?}, diagnostics: {diags:?}",
        tests = all_tests(&report),
        msgs = iterations
            .iter()
            .flat_map(|i| i.tests.iter().map(|t| t.message.clone()))
            .collect::<Vec<_>>(),
        diags = iterations
            .iter()
            .flat_map(|i| i.diagnostics.iter().map(|d| d.message.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(report.tests_failed, 0);
    assert!(report.ok());
    assert_eq!(report.logs.len(), 3);
    assert!(report.logs[2].contains("Rodriquez"), "{:?}", report.logs);
    for echo in &echoes {
        echo.assert_hits(1);
    }
}

#[tokio::test]
async fn loop_reads_json_values_from_the_environment_file_layer() {
    let server = MockServer::start();
    let echoes = mock_client_echoes(&server);
    let tmp = tempfile::tempdir().unwrap();

    // Same loop, but `clients` comes from the environment instead of a script.
    let src = "\
### loop from env
POST <BASE>/post
Content-Type: application/json

{
  \"clientId\": {{$.clients..id}},
  \"firstName\": \"{{$.clients..firstName}}\",
  \"balance\": \"{{$.clients..balance}}\"
}
"
    .replace("<BASE>", &server.base_url());

    let mut env = Environment::new();
    env.set_json(
        http_core::env::Layer::Environment,
        "clients",
        serde_json::json!([
            { "id": 1, "firstName": "George", "lastName": "Franklin", "balance": 100 },
            { "id": 2, "firstName": "John", "lastName": "Doe", "balance": 1500 },
            { "id": 3, "firstName": "Eduardo", "lastName": "Rodriquez", "balance": 10 }
        ]),
    );

    let report = run(&src, env, tmp.path()).await;
    assert_eq!(report.requests[0].iterations.len(), 3);
    for echo in &echoes {
        echo.assert_hits(1);
    }
}

// --- P7-14: `client.exit()` stops the file ---

#[tokio::test]
async fn client_exit_stops_remaining_requests() {
    let server = MockServer::start();
    let first = server.mock(|when, then| {
        when.method(Method::GET).path("/one");
        then.status(200).body("1");
    });
    let second = server.mock(|when, then| {
        when.method(Method::GET).path("/two");
        then.status(200).body("2");
    });
    let tmp = tempfile::tempdir().unwrap();

    let src = "\
### one
GET <BASE>/one

> {%
    client.test(\"reached one\", () => { client.assert(response.status === 200) })
    client.exit()
%}

### two
GET <BASE>/two
"
    .replace("<BASE>", &server.base_url());

    let report = run(&src, Environment::new(), tmp.path()).await;

    assert_eq!(report.requests.len(), 1, "the second request never ran");
    assert_eq!(report.tests_passed, 1);
    first.assert_hits(1);
    second.assert_hits(0);
}

// --- P7-14: `>>` / `>>!` land next to the run's base directory ---

#[tokio::test]
async fn output_redirects_are_written_for_each_request() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(Method::GET).path("/first");
        then.status(200).header("Content-Type", "application/json").body("{\"n\":1}");
    });
    server.mock(|when, then| {
        when.method(Method::GET).path("/second");
        then.status(200).header("Content-Type", "application/json").body("{\"n\":2}");
    });
    let tmp = tempfile::tempdir().unwrap();
    // Pre-existing target: `>>!` must overwrite it.
    std::fs::write(tmp.path().join("forced.json"), b"stale").unwrap();

    let src = "\
### save response
GET <BASE>/first

>> saved.json

### force save response
GET <BASE>/second

>>! forced.json
"
    .replace("<BASE>", &server.base_url());

    run(&src, Environment::new(), tmp.path()).await;

    assert_eq!(
        std::fs::read_to_string(tmp.path().join("saved.json")).unwrap(),
        "{\"n\":1}"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("forced.json")).unwrap(),
        "{\"n\":2}",
        "force redirect overwrote the stale file"
    );
}

#[tokio::test]
async fn history_folder_redirect_is_rooted_at_the_base_dir() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(Method::GET).path("/get");
        then.status(200).body("body");
    });
    let tmp = tempfile::tempdir().unwrap();

    let src = "\
### own filename
GET <BASE>/get

>> {{$historyFolder}}/my-response.json
"
    .replace("<BASE>", &server.base_url());

    run(&src, Environment::new(), tmp.path()).await;

    let written = tmp.path().join(".http-history").join("my-response.json");
    assert_eq!(std::fs::read_to_string(&written).unwrap(), "body");
}

// --- P7-14 / P7-15: `--request` style filtering ---

#[tokio::test]
async fn request_filter_selects_by_name_or_position() {
    let server = MockServer::start();
    let a = server.mock(|when, then| {
        when.method(Method::GET).path("/a");
        then.status(200).body("a");
    });
    let b = server.mock(|when, then| {
        when.method(Method::GET).path("/b");
        then.status(200).body("b");
    });
    let c = server.mock(|when, then| {
        when.method(Method::GET).path("/c");
        then.status(200).body("c");
    });
    let tmp = tempfile::tempdir().unwrap();

    let src = "\
### alpha
GET <BASE>/a

### beta
GET <BASE>/b

### gamma
GET <BASE>/c
"
    .replace("<BASE>", &server.base_url());

    let by_name = run_with_filter(&src, Environment::new(), tmp.path(), Some("beta")).await;
    assert_eq!(by_name.requests.len(), 1);
    assert_eq!(by_name.requests[0].name.as_deref(), Some("beta"));

    let by_index = run_with_filter(&src, Environment::new(), tmp.path(), Some("3")).await;
    assert_eq!(by_index.requests.len(), 1);
    assert_eq!(by_index.requests[0].name.as_deref(), Some("gamma"));

    let unknown = run_with_filter(&src, Environment::new(), tmp.path(), Some("nope")).await;
    assert!(unknown.requests.is_empty());

    // `/a` is never selected by any of the three filters above.
    a.assert_hits(0);
    b.assert_hits(1);
    c.assert_hits(1);
}

// --- P7-14: `run_file` reads from disk and roots refs at the file's parent ---

#[tokio::test]
async fn run_file_resolves_handler_refs_next_to_the_http_file() {
    let server = MockServer::start();
    let hit = server.mock(|when, then| {
        when.method(Method::GET)
            .path("/with-handler-file")
            .header("X-From-Script", "yes");
        then.status(200)
            .header("Content-Type", "application/json")
            .body("{\"ok\":true}");
    });

    let tmp = tempfile::tempdir().unwrap();
    // The response handler lives in an external file next to the `.http` file.
    std::fs::write(
        tmp.path().join("check.js"),
        "client.test(\"body is ok\", () => { client.assert(response.body.ok === true) })\n",
    )
    .unwrap();
    let http_path = tmp.path().join("run.http");
    std::fs::write(
        &http_path,
        "\
### external handler file
< {%
    request.variables.set(\"flag\", \"yes\")
%}

GET <BASE>/with-handler-file
X-From-Script: {{flag}}

> ./check.js
>> out.json
"
        .replace("<BASE>", &server.base_url()),
    )
    .unwrap();

    // `base_dir` is inferred from the file's parent, so `./check.js` and
    // `>> out.json` both resolve inside the temp dir.
    let report = run_file_with(
        &http_path,
        Environment::new(),
        &RunOptions::default(),
        &Dispatcher::new(),
    )
    .await
    .expect("run_file ok");

    assert_eq!(report.tests_passed, 1);
    assert_eq!(report.tests_failed, 0);
    assert!(report.ok());
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("out.json")).unwrap(),
        "{\"ok\":true}"
    );
    hit.assert_hits(1);
}
