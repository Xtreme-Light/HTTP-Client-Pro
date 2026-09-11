//! TDD tests for http-cli Phase 5 P5-2.
//!
//! Strategy: drive the public API `parse_args` + `Cli::run` (in-process,
//! async) against an `httpmock` upstream. Use `tempfile` to drop the
//! `.http` source and `http-client.env.json` into a working dir.
//!
//! Note on output capture: `Cli::run` writes to stdout via `println!`,
//! which we cannot easily intercept in-unit. These tests focus on the
//! observable side-effect — exit code and (where relevant) the upstream
//! mock being hit — plus unit coverage of `select_request` / `load_env`
//! whose return values are directly inspectable.

use http_cli::{load_env, parse_args, select_request, CliError};
use http_core::env::Environment;
use http_core::model::{Method, Request, RequestLine, RequestTarget};
use httpmock::MockServer;
use std::path::PathBuf;
use tempfile::TempDir;

fn make_request(name: Option<&str>, method: Method, target: &str) -> Request {
    Request {
        line: RequestLine {
            method,
            target: RequestTarget::Absolute {
                scheme: Some("http".to_string()),
                authority: "example.com".to_string(),
                path: Some(target.to_string()),
                query: None,
                fragment: None,
            },
            http_version: None,
        },
        headers: Vec::new(),
        body: None,
        response_handler: None,
        response_ref: None,
        pre_request_script: None,
        output_redirect: None,
        tags: Vec::new(),
        name: name.map(str::to_string),
    }
}

#[test]
fn select_first_when_no_selection() {
    let reqs = vec![
        make_request(None, Method::Get, "/a"),
        make_request(None, Method::Get, "/b"),
    ];
    let selected = select_request(&reqs, None).expect("first");
    assert!(selected.name.is_none());
    let RequestTarget::Absolute { path, .. } = &selected.line.target else {
        panic!("expected absolute target");
    };
    assert_eq!(path.as_deref(), Some("/a"));
}

#[test]
fn select_by_name_matches_separator_comment() {
    let reqs = vec![
        make_request(None, Method::Get, "/a"),
        make_request(Some("login"), Method::Post, "/auth"),
        make_request(None, Method::Get, "/c"),
    ];
    let selected = select_request(&reqs, Some("login")).expect("found by name");
    let RequestTarget::Absolute { path, .. } = &selected.line.target else {
        panic!("expected absolute target");
    };
    assert_eq!(path.as_deref(), Some("/auth"));
}

#[test]
fn select_by_numeric_index_one_based() {
    let reqs = vec![
        make_request(None, Method::Get, "/a"),
        make_request(None, Method::Get, "/b"),
        make_request(None, Method::Get, "/c"),
    ];
    let selected = select_request(&reqs, Some("2")).expect("index 2");
    let RequestTarget::Absolute { path, .. } = &selected.line.target else {
        panic!("expected absolute target");
    };
    assert_eq!(path.as_deref(), Some("/b"));
}

#[test]
fn select_unknown_name_returns_error() {
    let reqs = vec![make_request(None, Method::Get, "/a")];
    let err = select_request(&reqs, Some("missing")).unwrap_err();
    assert!(matches!(err, CliError::RequestNotFound(_)));
}

#[test]
fn select_empty_file_returns_error() {
    let reqs: Vec<Request> = Vec::new();
    let err = select_request(&reqs, None).unwrap_err();
    assert!(matches!(err, CliError::RequestNotFound(_)));
}

#[test]
fn parse_args_run_with_positional_file() {
    let args = vec![
        "http-client-pro".to_string(),
        "run".to_string(),
        "test.http".to_string(),
    ];
    let cli = parse_args(args).expect("parse");
    let http_cli::Subcommand::Run(run) = cli.subcommand;
    assert_eq!(run.file, PathBuf::from("test.http"));
    assert!(!run.json);
    assert!(run.request.is_none());
    assert!(run.env.is_none());
}

#[test]
fn parse_args_run_with_all_flags() {
    let args = vec![
        "http-client-pro".to_string(),
        "run".to_string(),
        "test.http".to_string(),
        "--request".to_string(),
        "login".to_string(),
        "--env".to_string(),
        "staging".to_string(),
        "--json".to_string(),
    ];
    let cli = parse_args(args).expect("parse");
    let http_cli::Subcommand::Run(run) = cli.subcommand;
    assert_eq!(run.file, PathBuf::from("test.http"));
    assert_eq!(run.request.as_deref(), Some("login"));
    assert_eq!(run.env.as_deref(), Some("staging"));
    assert!(run.json);
}

#[test]
fn parse_args_missing_subcommand_is_usage_error() {
    let args = vec!["http-client-pro".to_string()];
    let err = parse_args(args).unwrap_err();
    assert!(matches!(err, CliError::Usage));
}

#[test]
fn parse_args_missing_file_is_usage_error() {
    let args = vec!["http-client-pro".to_string(), "run".to_string()];
    let err = parse_args(args).unwrap_err();
    assert!(matches!(err, CliError::Usage));
}

#[test]
fn load_env_returns_empty_when_no_name() {
    let env = load_env(None, None, std::path::Path::new(".")).expect("empty");
    assert!(env.is_empty());
}

#[test]
fn load_env_loads_named_environment_from_default_path() {
    let tmp = TempDir::new().unwrap();
    let env_path = tmp.path().join("http-client.env.json");
    std::fs::write(
        &env_path,
        r#"{
            "staging": { "host": "staging.example.com" },
            "prod":    { "host": "prod.example.com" }
        }"#,
    )
    .unwrap();
    let env = load_env(Some("staging"), None, tmp.path()).expect("loaded");
    assert_eq!(env.get("host").as_deref(), Some("staging.example.com"));
}

#[test]
fn load_env_explicit_path_overrides_default() {
    let tmp = TempDir::new().unwrap();
    let custom = tmp.path().join("custom.env.json");
    std::fs::write(&custom, r#"{ "dev": { "token": "abc" } }"#).unwrap();
    let env = load_env(Some("dev"), Some(&custom), tmp.path()).expect("loaded");
    assert_eq!(env.get("token").as_deref(), Some("abc"));
}

#[test]
fn load_env_unknown_name_errors() {
    let tmp = TempDir::new().unwrap();
    let env_path = tmp.path().join("http-client.env.json");
    std::fs::write(&env_path, r#"{ "staging": {} }"#).unwrap();
    let err = load_env(Some("prod"), None, tmp.path()).unwrap_err();
    assert!(matches!(err, CliError::EnvNotFound(_, _)));
}

#[test]
fn load_env_missing_file_errors() {
    let tmp = TempDir::new().unwrap();
    let err = load_env(Some("staging"), None, tmp.path()).unwrap_err();
    assert!(matches!(err, CliError::EnvFile(_, _)));
}

#[tokio::test]
async fn run_executes_first_request_against_mock() {
    let upstream = MockServer::start();
    let m = upstream.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/api/get");
        then.status(200).body("hello-cli");
    });

    let tmp = TempDir::new().unwrap();
    let http_file = tmp.path().join("req.http");
    std::fs::write(&http_file, format!("GET {}\n", upstream.url("/api/get"))).unwrap();

    let args = vec![
        "http-client-pro".to_string(),
        "run".to_string(),
        http_file.to_string_lossy().to_string(),
    ];
    let cli = parse_args(args).expect("parse");
    // `Cli::run` returns ExitCode; we can't read stdout here, but the
    // upstream mock's hit count is observable proof that the request fired.
    let code = cli.run().await;
    assert!(code_eq_success(code));
    m.assert_hits(1);
}

#[tokio::test]
async fn run_with_request_name_selects_named_request() {
    let upstream = MockServer::start();
    let login_mock = upstream.mock(|when, then| {
        when.method(httpmock::Method::POST).path("/auth/login");
        then.status(200).body("tok");
    });
    // First request hits a non-mocked path, so it would 404 — but we don't
    // call it because --request selects the second.
    let never_mock = upstream.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/never-called");
        then.status(404);
    });

    let tmp = TempDir::new().unwrap();
    let http_file = tmp.path().join("req.http");
    std::fs::write(
        &http_file,
        format!(
            "GET {}\n\n### login\nPOST {}\n",
            upstream.url("/never-called"),
            upstream.url("/auth/login"),
        ),
    )
    .unwrap();

    let args = vec![
        "http-client-pro".to_string(),
        "run".to_string(),
        http_file.to_string_lossy().to_string(),
        "--request".to_string(),
        "login".to_string(),
    ];
    let cli = parse_args(args).expect("parse");
    let code = cli.run().await;
    assert!(code_eq_success(code));
    // The /never-called endpoint should not have been hit, only /auth/login.
    login_mock.assert_hits(1);
    never_mock.assert_hits(0);
}

#[tokio::test]
async fn run_with_env_substitutes_variables() {
    let upstream = MockServer::start();
    let m = upstream.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/api/v1/items");
        then.status(200).body("ok");
    });

    let tmp = TempDir::new().unwrap();
    // Strip the `http://` scheme so the env value is host:port only;
    // the scheme is written in the .http source so the parser treats
    // the URL as absolute-form (authority = {{host}}).
    let host = upstream.url("").trim_start_matches("http://").to_string();
    // The .http source uses {{host}} (in absolute-form URL) from the env file.
    let http_file = tmp.path().join("req.http");
    std::fs::write(&http_file, "GET http://{{host}}/api/v1/items\n").unwrap();
    let env_file = tmp.path().join("http-client.env.json");
    std::fs::write(
        &env_file,
        format!(r#"{{ "staging": {{ "host": "{}" }} }}"#, host),
    )
    .unwrap();

    let args = vec![
        "http-client-pro".to_string(),
        "run".to_string(),
        http_file.to_string_lossy().to_string(),
        "--env".to_string(),
        "staging".to_string(),
    ];
    let cli = parse_args(args).expect("parse");
    let code = cli.run().await;
    assert!(code_eq_success(code));
    m.assert_hits(1);
}

#[tokio::test]
async fn run_executes_every_request_in_the_file_by_default() {
    let upstream = MockServer::start();
    let first = upstream.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/one");
        then.status(200).body("1");
    });
    let second = upstream.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/two");
        then.status(200).body("2");
    });

    let tmp = TempDir::new().unwrap();
    let http_file = tmp.path().join("two.http");
    std::fs::write(
        &http_file,
        format!(
            "GET {}\n\n###\nGET {}\n",
            upstream.url("/one"),
            upstream.url("/two")
        ),
    )
    .unwrap();

    // `--json` only changes the report printer; both requests must still run.
    let args = vec![
        "http-client-pro".to_string(),
        "run".to_string(),
        http_file.to_string_lossy().to_string(),
        "--json".to_string(),
    ];
    let cli = parse_args(args).expect("parse");
    assert!(code_eq_success(cli.run().await));
    first.assert_hits(1);
    second.assert_hits(1);
}

#[tokio::test]
async fn run_exits_non_zero_when_a_client_test_fails() {
    let upstream = MockServer::start();
    upstream.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/boom");
        then.status(500).body("nope");
    });

    let tmp = TempDir::new().unwrap();
    let http_file = tmp.path().join("boom.http");
    std::fs::write(
        &http_file,
        format!(
            "GET {}\n\n> {{% client.test(\"status is 200\", function() {{ \
             client.assert(response.status === 200, \"not 200\"); }}); %}}\n",
            upstream.url("/boom")
        ),
    )
    .unwrap();

    let args = vec![
        "http-client-pro".to_string(),
        "run".to_string(),
        http_file.to_string_lossy().to_string(),
    ];
    let cli = parse_args(args).expect("parse");
    assert!(
        !code_eq_success(cli.run().await),
        "a failed client.test must surface as exit code 1"
    );
}

fn code_eq_success(c: std::process::ExitCode) -> bool {
    // ExitCode eq with SUCCESS is stable since 1.61.
    c == std::process::ExitCode::SUCCESS
}

// `Environment` is referenced just to keep the import explicit — proves
// the public surface is reachable for downstream integrators.
#[allow(dead_code)]
fn _env_type_check(_e: Environment) {}
