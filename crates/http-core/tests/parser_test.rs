//! Parser tests — spec chapter 3.
//!
//! The parser consumes a lexed `Vec<Line>` stream (or a raw string via
//! [`parse_file`]) and produces a [`RequestsFile`]. These tests target:
//! - P1-3 requests file (spec 3.1)
//! - P1-4 request line  (spec 3.2.1)
//! - P1-5 request target (spec 3.2.1.1~3.2.1.4)
//!
//! Headers / body / multipart / handler / env-var tests live in dedicated
//! files; this file focuses on file structure and the request line.

use http_core::model::*;
use http_core::parser::parse_file;

fn absolute(scheme: Option<&str>, authority: &str, path: Option<&str>) -> RequestTarget {
    RequestTarget::Absolute {
        scheme: scheme.map(str::to_string),
        authority: authority.to_string(),
        path: path.map(str::to_string),
        query: None,
        fragment: None,
    }
}

// --- P1-3: requests file (spec 3.1) ---

#[test]
fn empty_file_yields_zero_requests() {
    let f = parse_file("").unwrap();
    assert!(f.requests.is_empty());
}

#[test]
fn only_separators_yields_zero_requests() {
    let f = parse_file("###\n###\n###\n").unwrap();
    assert!(f.requests.is_empty());
}

#[test]
fn only_comments_yields_zero_requests() {
    let f = parse_file("# just a comment\n// another\n").unwrap();
    assert!(f.requests.is_empty());
}

#[test]
fn single_request_no_separator() {
    let f = parse_file("GET http://example.com\n").unwrap();
    assert_eq!(f.requests.len(), 1);
    assert_eq!(f.requests[0].line.method, Method::Get);
}

#[test]
fn two_requests_separated() {
    let src = "###\nGET http://a.com\n###\nGET http://b.com\n###\n";
    let f = parse_file(src).unwrap();
    assert_eq!(f.requests.len(), 2);
}

#[test]
fn leading_and_trailing_separators_optional() {
    // spec 3.1: file may start or end with multiple request separators.
    let src = "###\n###\nGET http://a.com\n###\n###\n";
    let f = parse_file(src).unwrap();
    assert_eq!(f.requests.len(), 1);
}

#[test]
fn consecutive_separators_do_not_create_empty_requests() {
    let src = "GET http://a.com\n###\n###\n###\nGET http://b.com\n";
    let f = parse_file(src).unwrap();
    assert_eq!(f.requests.len(), 2);
}

// --- P1-4: request line (spec 3.2.1) ---

#[test]
fn default_method_is_get() {
    let f = parse_file("http://example.com\n").unwrap();
    assert_eq!(f.requests[0].line.method, Method::Get);
}

#[test]
fn all_methods_recognized() {
    for (text, expected) in [
        ("GET", Method::Get),
        ("HEAD", Method::Head),
        ("POST", Method::Post),
        ("PUT", Method::Put),
        ("DELETE", Method::Delete),
        ("CONNECT", Method::Connect),
        ("PATCH", Method::Patch),
        ("OPTIONS", Method::Options),
        ("TRACE", Method::Trace),
    ] {
        let src = format!("{text} http://example.com\n");
        let f = parse_file(&src).unwrap_or_else(|_| panic!("parse failed for {text}"));
        assert_eq!(f.requests[0].line.method, expected, "method {text}");
    }
}

#[test]
fn http_version_parsed() {
    let f = parse_file("GET http://example.com HTTP/1.1\n").unwrap();
    assert_eq!(f.requests[0].line.http_version.as_deref(), Some("HTTP/1.1"));
}

#[test]
fn http_version_omitted_by_default() {
    let f = parse_file("GET http://example.com\n").unwrap();
    assert!(f.requests[0].line.http_version.is_none());
}

#[test]
fn invalid_method_is_diagnostic() {
    // Lenient parse: bad method is recorded as a diagnostic, not a hard error.
    let out = parse_file("FOO http://example.com\n").expect("parse always Ok");
    assert!(out.requests.is_empty());
    assert_eq!(out.diagnostics.len(), 1);
    assert_eq!(out.diagnostics[0].kind, http_core::error::ErrorKind::Parse);
}

// --- P1-5: request target (spec 3.2.1.1~3.2.1.4) ---

#[test]
fn absolute_form_full() {
    let f = parse_file("GET http://example.com/api/get\n").unwrap();
    assert_eq!(
        f.requests[0].line.target,
        absolute(Some("http"), "example.com", Some("/api/get"))
    );
}

#[test]
fn absolute_form_scheme_defaults_to_http() {
    let f = parse_file("GET //example.com/api/get\n").unwrap();
    assert_eq!(
        f.requests[0].line.target,
        absolute(Some("http"), "example.com", Some("/api/get"))
    );
}

#[test]
fn https_scheme_recognized() {
    let f = parse_file("GET https://example.com/api/get\n").unwrap();
    assert_eq!(
        f.requests[0].line.target,
        absolute(Some("https"), "example.com", Some("/api/get"))
    );
}

#[test]
fn absolute_form_no_path() {
    let f = parse_file("GET http://example.com\n").unwrap();
    assert_eq!(
        f.requests[0].line.target,
        absolute(Some("http"), "example.com", None)
    );
}

#[test]
fn authority_with_port() {
    let f = parse_file("GET http://example.com:8080/api\n").unwrap();
    assert_eq!(
        f.requests[0].line.target,
        absolute(Some("http"), "example.com:8080", Some("/api"))
    );
}

#[test]
fn authority_ipv6() {
    let f = parse_file("GET http://[::1]/api\n").unwrap();
    assert_eq!(
        f.requests[0].line.target,
        absolute(Some("http"), "[::1]", Some("/api"))
    );
}

#[test]
fn authority_ipv6_with_port() {
    let f = parse_file("GET http://[::1]:8080/api\n").unwrap();
    assert_eq!(
        f.requests[0].line.target,
        absolute(Some("http"), "[::1]:8080", Some("/api"))
    );
}

#[test]
fn origin_form_path() {
    let f = parse_file("GET /api/get\nHost: example.com\n").unwrap();
    match &f.requests[0].line.target {
        RequestTarget::Origin {
            path,
            query,
            fragment,
        } => {
            assert_eq!(path, "/api/get");
            assert!(query.is_none());
            assert!(fragment.is_none());
        }
        other => panic!("expected Origin, got {other:?}"),
    }
}

#[test]
fn origin_form_with_query() {
    let f = parse_file("GET /api/get?id=42\nHost: example.com\n").unwrap();
    match &f.requests[0].line.target {
        RequestTarget::Origin { query, .. } => assert_eq!(query.as_deref(), Some("id=42")),
        other => panic!("expected Origin, got {other:?}"),
    }
}

#[test]
fn origin_form_with_fragment() {
    let f = parse_file("GET /api/get#q=hello+world\nHost: example.com\n").unwrap();
    match &f.requests[0].line.target {
        RequestTarget::Origin { fragment, .. } => {
            assert_eq!(fragment.as_deref(), Some("q=hello+world"))
        }
        other => panic!("expected Origin, got {other:?}"),
    }
}

#[test]
fn asterisk_form() {
    let f = parse_file("OPTIONS * HTTP/1.1\nHost: example.com\n").unwrap();
    assert_eq!(f.requests[0].line.target, RequestTarget::Asterisk);
}

#[test]
fn absolute_form_with_query_and_fragment() {
    let f = parse_file("GET http://example.com/api/get?id=42#frag\n").unwrap();
    match &f.requests[0].line.target {
        RequestTarget::Absolute {
            scheme,
            authority,
            path,
            query,
            fragment,
        } => {
            assert_eq!(scheme.as_deref(), Some("http"));
            assert_eq!(authority, "example.com");
            assert_eq!(path.as_deref(), Some("/api/get"));
            assert_eq!(query.as_deref(), Some("id=42"));
            assert_eq!(fragment.as_deref(), Some("frag"));
        }
        other => panic!("expected Absolute, got {other:?}"),
    }
}

#[test]
fn multiline_path_continuation_joined() {
    // spec 3.2.1.3 example: `http://example.com/` + `api` + `/get`
    let src = "http://example.com/\n    api\n    /get\n";
    let f = parse_file(src).unwrap();
    assert_eq!(
        f.requests[0].line.target,
        absolute(Some("http"), "example.com", Some("/api/get"))
    );
}

#[test]
fn unicode_path_preserved() {
    let f = parse_file("GET http://example.com/中文\n").unwrap();
    assert_eq!(
        f.requests[0].line.target,
        absolute(Some("http"), "example.com", Some("/中文"))
    );
}

#[test]
fn unicode_query_preserved() {
    let f = parse_file("GET http://example.com/api?q=你好\n").unwrap();
    match &f.requests[0].line.target {
        RequestTarget::Absolute { query, .. } => assert_eq!(query.as_deref(), Some("q=你好")),
        other => panic!("expected Absolute, got {other:?}"),
    }
}
