//! Parser tests — headers + message body (spec 3.2.2 / 3.2.3).
//!
//! Covers:
//! - P1-6 headers: single, multiple, value trim, value continuation,
//!   case-insensitive name preservation, `field-name` any char except `:`.
//! - P1-7 message body: in-place, empty-line separator, leading/trailing
//!   trim (spec 4.2.2), file ref (`< file`), file body preserved verbatim,
//!   body terminator detection (`<`, `<> `, `###`).

use http_core::model::*;
use http_core::parser::parse_file;

fn header<'a>(req: &'a Request, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str())
}

fn first_req(src: &str) -> Request {
    let f = parse_file(src).expect("parse");
    f.requests.into_iter().next().expect("at least one request")
}

// --- P1-6: headers (spec 3.2.2) ---

#[test]
fn single_header() {
    let req = first_req("GET http://example.com\nFrom: user@example.com\n");
    assert_eq!(header(&req, "From"), Some("user@example.com"));
}

#[test]
fn multiple_headers_preserve_order() {
    let req = first_req(
        "POST http://example.com\nContent-Type: application/json\nAuthorization: Bearer xyz\n",
    );
    assert_eq!(req.headers.len(), 2);
    assert_eq!(req.headers[0].name, "Content-Type");
    assert_eq!(req.headers[1].name, "Authorization");
}

#[test]
fn header_value_leading_trailing_whitespace_trimmed() {
    // spec 3.2.2: `field-name ':' optional-whitespace field-value optional-whitespace`
    let req = first_req("GET http://example.com\nX-Custom:    spaced value    \n");
    assert_eq!(header(&req, "X-Custom"), Some("spaced value"));
}

#[test]
fn header_value_continuation_joined() {
    // spec 3.2.2 `field-value: line-tail [new-line-with-indent field-value]`
    let req = first_req("GET http://example.com\nX-Long: first part\n    second part\n");
    assert_eq!(header(&req, "X-Long"), Some("first part second part"));
}

#[test]
fn header_name_preserves_case_for_display() {
    // Comparison is case-insensitive; the original casing is kept for round-trip.
    let req = first_req("GET http://example.com\ncOnTeNt-TyPe: application/json\n");
    assert_eq!(req.headers[0].name, "cOnTeNt-TyPe");
    assert_eq!(header(&req, "content-type"), Some("application/json"));
}

#[test]
fn header_name_allows_any_char_except_colon() {
    // spec 3.2.2 `field-name: (any input-character except ':')+`
    // A name containing digits and dashes is the common case; we also allow
    // unusual but valid chars like `.` or `_`.
    let req = first_req("GET http://example.com\nX.Custom_Header: v\n");
    assert_eq!(req.headers[0].name, "X.Custom_Header");
}

#[test]
fn no_headers_yields_empty_list() {
    let req = first_req("GET http://example.com\n");
    assert!(req.headers.is_empty());
}

// --- P1-7: message body (spec 3.2.3) ---

#[test]
fn inline_body_captured() {
    let src =
        "POST http://example.com/add\nContent-Type: application/json\n\n{ \"key\": \"value\" }\n";
    let req = first_req(src);
    match req.body {
        Some(MessageBody::Inline { content }) => {
            assert_eq!(content, "{ \"key\": \"value\" }");
        }
        other => panic!("expected Inline body, got {other:?}"),
    }
}

#[test]
fn inline_body_internal_newlines_preserved() {
    let src = "POST http://example.com/add\nContent-Type: application/json\n\n{\n  \"key\": \"value\"\n}\n";
    let req = first_req(src);
    match req.body {
        Some(MessageBody::Inline { content }) => {
            assert_eq!(content, "{\n  \"key\": \"value\"\n}");
        }
        other => panic!("expected Inline body, got {other:?}"),
    }
}

#[test]
fn inline_body_leading_trailing_trimmed() {
    // spec 4.2.2: in-place body whitespace around it is trimmed.
    let src = "###\nPOST http://example.com/add\n\n\n\nmessage-body\n\n###\n";
    let req = first_req(src);
    match req.body {
        Some(MessageBody::Inline { content }) => assert_eq!(content, "message-body"),
        other => panic!("expected Inline body, got {other:?}"),
    }
}

#[test]
fn no_body_yields_none() {
    let req = first_req("GET http://example.com\n");
    assert!(req.body.is_none());
}

#[test]
fn body_with_no_headers_still_parsed() {
    // No headers; empty line directly after request line starts the body.
    let src = "POST http://example.com/add\n\nraw body\n";
    let req = first_req(src);
    match req.body {
        Some(MessageBody::Inline { content }) => assert_eq!(content, "raw body"),
        other => panic!("expected Inline body, got {other:?}"),
    }
}

#[test]
fn body_file_ref_captured() {
    // spec 3.2.3 `input-file-ref: '<' required-whitespace file-path`
    let src = "POST http://example.com/add\nContent-Type: application/json\n\n< ./input.json\n";
    let req = first_req(src);
    match req.body {
        Some(MessageBody::FileRef { path }) => assert_eq!(path, "./input.json"),
        other => panic!("expected FileRef body, got {other:?}"),
    }
}

#[test]
fn body_file_ref_with_extra_whitespace_in_path() {
    // file-path = line-tail — keep as-is after the required whitespace.
    let src = "POST http://example.com/add\n\n<   data/input body.json\n";
    let req = first_req(src);
    match req.body {
        Some(MessageBody::FileRef { path }) => assert_eq!(path, "data/input body.json"),
        other => panic!("expected FileRef body, got {other:?}"),
    }
}

#[test]
fn body_terminator_at_separator() {
    let src = "###\nPOST http://example.com/add\n\nfirst body line\n###\nGET http://next.com\n";
    let f = parse_file(src).unwrap();
    assert_eq!(f.requests.len(), 2);
    match &f.requests[0].body {
        Some(MessageBody::Inline { content }) => assert_eq!(content, "first body line"),
        other => panic!("expected Inline body, got {other:?}"),
    }
}

#[test]
fn body_terminator_at_response_handler() {
    // Body stops at the response-handler marker `> ` (spec 3.2.3 message-line).
    let src = "GET http://example.com/auth\n\nbody line\n> {% client.global.set(\"x\", 1); %}\n";
    let req = first_req(src);
    match req.body {
        Some(MessageBody::Inline { content }) => assert_eq!(content, "body line"),
        other => panic!("expected Inline body, got {other:?}"),
    }
}

#[test]
fn body_terminator_at_response_ref() {
    let src = "GET http://example.com\n\nbody line\n<> previous-response.json\n";
    let req = first_req(src);
    match req.body {
        Some(MessageBody::Inline { content }) => assert_eq!(content, "body line"),
        other => panic!("expected Inline body, got {other:?}"),
    }
}
