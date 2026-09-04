//! Parser tests — multipart/form-data (spec 3.2.3.1).
//!
//! Covers:
//! - P1-8 single multipart-field, multiple fields, ending boundary
//!   `--abcd--`, field body as inline or `< file` ref, missing
//!   `Content-Type: multipart/form-data; boundary=...` is a parse error.

use http_core::model::*;
use http_core::parser::parse_file;

fn first_req(src: &str) -> Request {
    let f = parse_file(src).expect("parse");
    f.requests.into_iter().next().expect("one request")
}

const SIMPLE_MULTIPART: &str = "\
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

#[test]
fn multipart_detected_from_content_type() {
    let req = first_req(SIMPLE_MULTIPART);
    match req.body {
        Some(MessageBody::Multipart { boundary, parts }) => {
            assert_eq!(boundary, "abcd");
            assert_eq!(parts.len(), 2);
        }
        other => panic!("expected Multipart body, got {other:?}"),
    }
}

#[test]
fn multipart_field_headers_captured() {
    let req = first_req(SIMPLE_MULTIPART);
    if let Some(MessageBody::Multipart { parts, .. }) = req.body {
        assert_eq!(parts[0].headers.len(), 1);
        assert_eq!(parts[0].headers[0].name, "Content-Disposition");
        assert_eq!(parts[0].headers[0].value, "form-data; name=\"text\"");
    } else {
        panic!("expected Multipart");
    }
}

#[test]
fn multipart_inline_part_body() {
    let req = first_req(SIMPLE_MULTIPART);
    if let Some(MessageBody::Multipart { parts, .. }) = req.body {
        match &parts[0].body {
            MessagePartBody::Inline { content } => assert_eq!(content, "Text"),
            other => panic!("expected Inline part body, got {other:?}"),
        }
    } else {
        panic!("expected Multipart");
    }
}

#[test]
fn multipart_file_ref_part_body() {
    let req = first_req(SIMPLE_MULTIPART);
    if let Some(MessageBody::Multipart { parts, .. }) = req.body {
        match &parts[1].body {
            MessagePartBody::FileRef { path } => assert_eq!(path, "./input.txt"),
            other => panic!("expected FileRef part body, got {other:?}"),
        }
    } else {
        panic!("expected Multipart");
    }
}

#[test]
fn multipart_single_field() {
    let src = "\
POST http://example.com/api/upload
Content-Type: multipart/form-data; boundary=xyz

--xyz
Content-Disposition: form-data; name=\"text\"

hello
--xyz--
";
    let req = first_req(src);
    if let Some(MessageBody::Multipart { boundary, parts }) = req.body {
        assert_eq!(boundary, "xyz");
        assert_eq!(parts.len(), 1);
        match &parts[0].body {
            MessagePartBody::Inline { content } => assert_eq!(content, "hello"),
            other => panic!("expected Inline part body, got {other:?}"),
        }
    } else {
        panic!("expected Multipart");
    }
}

#[test]
fn multipart_without_boundary_in_content_type_is_diagnostic() {
    let src = "\
POST http://example.com/api/upload
Content-Type: multipart/form-data

--abcd
Content-Disposition: form-data; name=\"text\"

Text
--abcd--
";
    // Lenient parse: missing boundary is recorded as a diagnostic and the
    // request is skipped (per spec 3.2.3.1 boundary is mandatory).
    let out = parse_file(src).expect("parse always Ok");
    assert!(
        out.requests.is_empty(),
        "malformed request should be skipped"
    );
    assert_eq!(out.diagnostics.len(), 1);
    assert_eq!(out.diagnostics[0].kind, http_core::error::ErrorKind::Parse);
    assert!(out.diagnostics[0].message.contains("boundary"));
}

#[test]
fn multipart_without_multipart_content_type_is_not_multipart() {
    // Body that looks like multipart but lacks the Content-Type header should
    // parse as inline text, not as a multipart body.
    let src = "\
POST http://example.com/api/upload

--abcd
Content-Disposition: form-data; name=\"text\"

Text
--abcd--
";
    let req = first_req(src);
    match req.body {
        Some(MessageBody::Inline { content }) => assert!(content.contains("--abcd")),
        other => panic!("expected Inline body, got {other:?}"),
    }
}
