//! Executor tests — spec chapter 4.1 (encoding), 4.2 (whitespace), 4.4 (env).
//!
//! Pure-logic tests for the preparation step that turns an AST [`Request`]
//! into a [`PreparedRequest`] ready to hand to reqwest. Dispatch against a
//! mock HTTP server lives in `dispatch_test.rs` (Phase 2b).

use http_core::env::Environment;
use http_core::execute::{encode_path, encode_query, prepare};
use http_core::parser::parse_file;

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

// --- P2-2: encoding (spec 4.1.2) ---

#[test]
fn encode_path_non_ascii_percent_encoded() {
    // spec 4.1.2: non-ASCII symbols in path are encoded before sending.
    assert_eq!(encode_path("/中文"), "/%E4%B8%AD%E6%96%87");
}

#[test]
fn encode_query_non_ascii_percent_encoded() {
    assert_eq!(encode_query("q=你好"), "q=%E4%BD%A0%E5%A5%BD");
}

#[test]
fn encode_already_encoded_not_double_encoded() {
    // spec 4.1.2: "the already encoded symbols must not be encoded twice."
    assert_eq!(encode_path("/api%20get"), "/api%20get");
}

#[test]
fn encode_user_writes_pct2520_sends_as_pct20() {
    // spec 4.1.2 example: `%20` must be inserted as `%2520` to be sent as
    // `%20` (i.e. the literal two characters '%', '2', '0').
    assert_eq!(encode_path("/api%2520get"), "/api%2520get");
}

#[test]
fn encode_path_preserves_ascii_unreserved() {
    // ASCII unreserved chars (A-Z a-z 0-9 - _ . ~) and structural `/` `?` `#`
    // are not encoded.
    assert_eq!(encode_path("/api/get_v1.2"), "/api/get_v1.2");
}

#[test]
fn encode_path_preserves_structural_chars() {
    // `/` `?` `#` are structural; the encoder leaves them alone so the
    // parser can split them later.
    assert_eq!(encode_path("/api/get?q=1#frag"), "/api/get?q=1#frag");
}

#[test]
fn encode_invalid_pct_sequence_left_as_is() {
    // `%zz` is not a valid percent-encoded sequence; spec is silent. We
    // preserve the literal characters (do not re-encode the `%`).
    assert_eq!(encode_path("/api%zz"), "/api%zz");
}

// --- P2-1: variable substitution applied to target / headers / body ---

#[test]
fn prepare_substitutes_target_authority() {
    let req = first_req("GET http://{{host}}/api/get\n");
    let env = env_of(&[("host", "example.com")]);
    let prep = prepare(&req, &env).unwrap();
    assert_eq!(prep.url, "http://example.com/api/get");
}

#[test]
fn prepare_substitutes_target_with_whitespace_in_braces() {
    let req = first_req("GET http://{{ host }}/api/get\n");
    let env = env_of(&[("host", "example.com")]);
    let prep = prepare(&req, &env).unwrap();
    assert_eq!(prep.url, "http://example.com/api/get");
}

#[test]
fn prepare_substitutes_query_value() {
    let req = first_req("GET http://example.com/api/get?id={{id}}\n");
    let env = env_of(&[("id", "42")]);
    let prep = prepare(&req, &env).unwrap();
    assert_eq!(prep.url, "http://example.com/api/get?id=42");
}

#[test]
fn prepare_substitutes_header_value() {
    let req = first_req("GET http://example.com\nAuthorization: Bearer {{token}}\n");
    let env = env_of(&[("token", "xyz")]);
    let prep = prepare(&req, &env).unwrap();
    let auth = prep
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("Authorization"))
        .unwrap();
    assert_eq!(auth.value, "Bearer xyz");
}

#[test]
fn prepare_substitutes_inline_body() {
    let req = first_req(
        "POST http://example.com/add\nContent-Type: application/json\n\n{ \"k\": \"{{v}}\" }\n",
    );
    let env = env_of(&[("v", "42")]);
    let prep = prepare(&req, &env).unwrap();
    let body = prep.body.expect("body present");
    assert_eq!(String::from_utf8_lossy(&body), "{ \"k\": \"42\" }");
}

#[test]
fn prepare_undefined_variable_preserved_in_url() {
    let req = first_req("GET http://{{unknown}}/api\n");
    let env = Environment::new();
    let prep = prepare(&req, &env).unwrap();
    assert_eq!(prep.url, "http://{{unknown}}/api");
}

// --- P2-2 + P2-1 combined: substituted non-ASCII gets encoded ---

#[test]
fn prepare_substituted_non_ascii_in_path_encoded() {
    let req = first_req("GET http://example.com/{{path}}\n");
    let env = env_of(&[("path", "中文")]);
    let prep = prepare(&req, &env).unwrap();
    assert_eq!(prep.url, "http://example.com/%E4%B8%AD%E6%96%87");
}

// --- P2-3: whitespace trimming (spec 4.2.1 / 4.2.2) ---

#[test]
fn prepare_multiline_path_segments_trimmed_and_joined() {
    // spec 3.2.1.3 + 4.2.1 example.
    let req = first_req("http://example.com/\n    api\n    /get\n");
    let prep = prepare(&req, &Environment::new()).unwrap();
    assert_eq!(prep.url, "http://example.com/api/get");
}

#[test]
fn prepare_inline_body_trimmed() {
    // spec 4.2.2: in-place body whitespace around it is trimmed.
    let req = first_req("###\nPOST http://example.com/add\n\n\n\nmessage-body\n\n###\n");
    let prep = prepare(&req, &Environment::new()).unwrap();
    let body = prep.body.expect("body present");
    assert_eq!(String::from_utf8_lossy(&body), "message-body");
}

// --- prepared request shape ---

#[test]
fn prepare_carries_method_and_version() {
    let req = first_req("GET http://example.com HTTP/1.1\n");
    let prep = prepare(&req, &Environment::new()).unwrap();
    assert_eq!(prep.method, http_core::model::Method::Get);
    assert_eq!(prep.http_version.as_deref(), Some("HTTP/1.1"));
}

#[test]
fn prepare_origin_form_requires_host_header() {
    // spec 3.2.1.1: origin-form must define Host header to be executable.
    // Without Host, prepare returns an error.
    let req = first_req("GET /api/get\n");
    let res = prepare(&req, &Environment::new());
    assert!(res.is_err());
}

#[test]
fn prepare_origin_form_with_host_succeeds() {
    let req = first_req("GET /api/get\nHost: example.com\n");
    let prep = prepare(&req, &Environment::new()).unwrap();
    assert_eq!(prep.url, "http://example.com/api/get");
}

#[test]
fn prepare_asterisk_form_preserved() {
    let req = first_req("OPTIONS * HTTP/1.1\nHost: example.com\n");
    let prep = prepare(&req, &Environment::new()).unwrap();
    assert_eq!(prep.url, "http://example.com*");
}

#[test]
fn prepare_body_default_utf8() {
    // spec 4.1.3: body encoding defaults to UTF-8 when Content-Type is unset.
    let req = first_req("POST http://example.com/add\n\nhéllo\n");
    let prep = prepare(&req, &Environment::new()).unwrap();
    let body = prep.body.expect("body present");
    assert_eq!(&body[..], "héllo".as_bytes());
}

// --- P2-4: multipart body construction (spec 4.3) ---

#[test]
fn prepare_multipart_body_assembled_with_boundaries() {
    // spec 3.2.3.1 / 4.3 example: string part + file part. The file part's
    // body is resolved at dispatch time (Phase 2b); prepare() leaves it
    // empty here but the structure is verifiable.
    let src = "\
POST http://example.com/api/upload
Content-Type: multipart/form-data; boundary=abcd

--abcd
Content-Disposition: form-data; name=\"text\"

Text
--abcd
Content-Disposition: form-data; name=\"file_to_send\"; filename=\"input.txt\"

< ./input.txt
--abcd--
";
    let req = first_req(src);
    let prep = prepare(&req, &Environment::new()).unwrap();
    assert_eq!(prep.multipart_boundary.as_deref(), Some("abcd"));
    let body = prep.body.expect("body present");
    let text = String::from_utf8_lossy(&body);
    // Each part starts with `--abcd` and ends with `\r\n`.
    assert!(text.starts_with("--abcd\r\n"));
    assert!(text.contains("Content-Disposition: form-data; name=\"text\""));
    assert!(text.contains("\r\n\r\nText\r\n"));
    assert!(text
        .contains("Content-Disposition: form-data; name=\"file_to_send\"; filename=\"input.txt\""));
    // Closing boundary.
    assert!(text.ends_with("--abcd--\r\n"));
}

#[test]
fn prepare_multipart_substitutes_env_vars_in_part_headers_and_bodies() {
    let src = "\
POST http://example.com/api/upload
Content-Type: multipart/form-data; boundary=b

--b
Content-Disposition: form-data; name=\"{{field}}\"

{{value}}
--b--
";
    let req = first_req(src);
    let mut env = Environment::new();
    env.set("field", "user");
    env.set("value", "hello");
    let prep = prepare(&req, &env).unwrap();
    let body = prep.body.expect("body present");
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("name=\"user\""));
    assert!(text.contains("\r\n\r\nhello\r\n"));
}
