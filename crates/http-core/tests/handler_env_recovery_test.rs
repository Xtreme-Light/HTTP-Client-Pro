//! Parser tests — response handler, response ref, env vars, error recovery.
//! spec 3.2.4 ~ 3.2.6 + general lenient-parse behaviour.

use http_core::error::ErrorKind;
use http_core::model::*;
use http_core::parser::parse_file;

fn first_req(src: &str) -> Request {
    let f = parse_file(src).expect("parse");
    f.requests.into_iter().next().expect("at least one request")
}

// --- P1-9: response handler (spec 3.2.4) ---

#[test]
fn response_handler_inline_captured() {
    let src = "GET http://example.com/auth\n\n> {% client.global.set(\"auth\", response.body.token); %}\n";
    let req = first_req(src);
    match req.response_handler {
        Some(ResponseHandler::Inline { script }) => {
            assert_eq!(script, "client.global.set(\"auth\", response.body.token);");
        }
        other => panic!("expected Inline handler, got {other:?}"),
    }
}

#[test]
fn response_handler_file_ref_captured() {
    let src = "GET http://example.com/auth\n\n> ./handler.js\n";
    let req = first_req(src);
    match req.response_handler {
        Some(ResponseHandler::FileRef { path }) => assert_eq!(path, "./handler.js"),
        other => panic!("expected FileRef handler, got {other:?}"),
    }
}

#[test]
fn response_handler_after_inline_body() {
    // Body terminator detection: handler must follow body correctly.
    let src = "POST http://example.com/auth\nContent-Type: application/json\n\n{ \"k\": 1 }\n> {% client.global.set(\"x\", 1); %}\n";
    let req = first_req(src);
    // Body is captured verbatim; handler is captured separately.
    match req.body {
        Some(MessageBody::Inline { content }) => assert_eq!(content, "{ \"k\": 1 }"),
        other => panic!("expected Inline body, got {other:?}"),
    }
    assert!(matches!(
        req.response_handler,
        Some(ResponseHandler::Inline { .. })
    ));
}

// --- P1-10: response ref (spec 3.2.5) ---

#[test]
fn response_ref_captured() {
    let src = "GET http://example.com\n\n<> previous-response.200.json\n";
    let req = first_req(src);
    assert_eq!(
        req.response_ref.as_deref(),
        Some("previous-response.200.json")
    );
}

#[test]
fn response_ref_after_handler() {
    // spec 3.2: `[message-body] [response-handler] [response-ref]`
    let src = "GET http://example.com\n\n> ./h.js\n<> previous.json\n";
    let req = first_req(src);
    assert!(matches!(
        req.response_handler,
        Some(ResponseHandler::FileRef { .. })
    ));
    assert_eq!(req.response_ref.as_deref(), Some("previous.json"));
}

// --- P1-11: environment variables (spec 3.2.6) ---

#[test]
fn env_variable_in_target_preserved() {
    let src = "GET http://{{host}}/api/get?id={{ element-id }}\n";
    let req = first_req(src);
    match req.line.target {
        RequestTarget::Absolute {
            authority, query, ..
        } => {
            assert_eq!(authority, "{{host}}");
            assert_eq!(query.as_deref(), Some("id={{ element-id }}"));
        }
        other => panic!("expected Absolute target, got {other:?}"),
    }
}

#[test]
fn env_variable_in_header_preserved() {
    let src = "GET http://example.com\nAuthorization: Bearer {{token}}\n";
    let req = first_req(src);
    let auth = req
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("Authorization"))
        .expect("Authorization header");
    assert_eq!(auth.value, "Bearer {{token}}");
}

#[test]
fn env_variable_in_body_preserved() {
    let src = "POST http://example.com/add\nContent-Type: application/json\n\n{ \"key\": \"{{value}}\" }\n";
    let req = first_req(src);
    match req.body {
        Some(MessageBody::Inline { content }) => assert_eq!(content, "{ \"key\": \"{{value}}\" }"),
        other => panic!("expected Inline body, got {other:?}"),
    }
}

#[test]
fn env_variable_identifier_with_dash_and_underscore() {
    // spec 2.1 `identifier` includes alpha/digit/'-'/'_'.
    let src = "GET http://{{host_name-1}}/api\n";
    let req = first_req(src);
    match req.line.target {
        RequestTarget::Absolute { authority, .. } => assert_eq!(authority, "{{host_name-1}}"),
        other => panic!("expected Absolute target, got {other:?}"),
    }
}

#[test]
fn env_variable_surrounding_whitespace_optional() {
    // spec 3.2.6 `{{ optional-whitespace identifier optional-whitespace }}`
    let src = "GET http://{{   host   }}/api\n";
    let req = first_req(src);
    match req.line.target {
        RequestTarget::Absolute { authority, .. } => assert_eq!(authority, "{{   host   }}"),
        other => panic!("expected Absolute target, got {other:?}"),
    }
}

// --- P1-12: error recovery ---

#[test]
fn bad_request_does_not_break_following_request() {
    let src = "###\nFOO http://bad.com\n###\nGET http://good.com\n";
    let out = parse_file(src).expect("parse");
    assert_eq!(out.requests.len(), 1);
    assert_eq!(out.requests[0].line.method, Method::Get);
    assert_eq!(out.diagnostics.len(), 1);
}

#[test]
fn multiple_bad_requests_yield_multiple_diagnostics() {
    let src = "###\nFOO http://bad1.com\n###\nBAR http://bad2.com\n###\nGET http://good.com\n";
    let out = parse_file(src).expect("parse");
    assert_eq!(out.requests.len(), 1, "one good request");
    assert_eq!(out.diagnostics.len(), 2, "two diagnostics");
}

#[test]
fn diagnostic_carries_kind_and_span() {
    let src = "FOO http://bad.com\n";
    let out = parse_file(src).expect("parse");
    assert_eq!(out.diagnostics.len(), 1);
    let d = &out.diagnostics[0];
    assert_eq!(d.kind, ErrorKind::Parse);
    // Span line is 1-based.
    assert!(d.span.line >= 1);
}

// --- Spec example snapshots (insta) ---

#[test]
fn snapshot_spec_1_2_example_request() {
    // spec 1.2 example request.
    let src = "###\nPOST http://example.com/api/add\nContent-Type: application/json\n\n{ \"name\": \"entity\", \"value\": \"content\" }\n";
    let out = parse_file(src).expect("parse");
    insta::assert_yaml_snapshot!(out);
}

#[test]
fn snapshot_spec_3_2_3_1_multipart_example() {
    // spec 3.2.3.1 multipart example.
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
    let out = parse_file(src).expect("parse");
    insta::assert_yaml_snapshot!(out);
}

#[test]
fn snapshot_spec_3_2_3_multipart_with_string_and_file_parts() {
    // spec 4.3 example: string part (no filename) and file part (with filename).
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
    let out = parse_file(src).expect("parse");
    insta::assert_yaml_snapshot!(out);
}

#[test]
fn snapshot_spec_3_2_full_request_with_handler_and_ref() {
    // spec 3.2 example: request with body file ref, handler, response ref.
    let src = "\
POST http://example.com/auth
Content-Type: application/json

< input.json
> {% client.global.set(\"auth\", response.body.token); %}
<> previous-response.200.json
";
    let out = parse_file(src).expect("parse");
    insta::assert_yaml_snapshot!(out);
}

#[test]
fn snapshot_spec_3_2_1_3_multiline_path() {
    // spec 3.2.1.3 + 4.2.1 example.
    let src = "http://example.com/\n    api\n    /get\n";
    let out = parse_file(src).expect("parse");
    insta::assert_yaml_snapshot!(out);
}

#[test]
fn snapshot_spec_3_2_6_env_variables() {
    // spec 3.2.6 example.
    let src = "GET http://{{host}}/api/get?id={{ element-id }}\n";
    let out = parse_file(src).expect("parse");
    insta::assert_yaml_snapshot!(out);
}
