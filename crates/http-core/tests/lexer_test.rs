//! Lexer tests — spec chapter 2.
//!
//! Design: lexer produces a stream of [`http_core::lexer::Line`] values.
//! Each physical line is classified as one of:
//! - `Separator { comment }`     — `###` (+ optional comment), spec 2.5
//! - `Comment { text }`           — `#` or `//` line comment, spec 2.4
//! - `Empty`                      — blank line (significant for headers/body split)
//! - `Indented { indent, text }` — new-line-with-indent continuation, spec 2.2
//! - `Content { text }`           — a normal content line
//!
//! New-line forms (LF / CR / CRLF) are normalized (spec 2.2).
//! Unicode is preserved in content/indent (spec 2.1).
//!
//! Tests normalize spans before comparing so call sites stay focused on
//! classification rather than exact line/column tracking.

use http_core::error::Span;
use http_core::lexer::{Lexer, Line};

fn lex(src: &str) -> Vec<Line> {
    Lexer::new(src).map(zero_span).collect()
}

/// Replace a line's span with `Span::new(0, 0)` so test fixtures don't need
/// to track exact locations.
fn zero_span(mut line: Line) -> Line {
    let zero = Span::new(0, 0);
    match &mut line {
        Line::Separator { span, .. }
        | Line::Comment { span, .. }
        | Line::Empty { span }
        | Line::Indented { span, .. }
        | Line::Content { span, .. } => *span = zero,
    }
    line
}

// --- P1-1: base symbols & line terminators ---

#[test]
fn empty_input_produces_no_lines() {
    assert_eq!(lex(""), Vec::<Line>::new());
}

#[test]
fn single_content_line_no_newline() {
    assert_eq!(
        lex("GET http://example.com"),
        vec![Line::content("GET http://example.com")]
    );
}

#[test]
fn content_line_with_trailing_newline_lf() {
    // LF normalization: trailing newline does not produce an extra Empty line.
    assert_eq!(lex("GET\n"), vec![Line::content("GET")]);
}

#[test]
fn content_line_with_trailing_crlf() {
    assert_eq!(lex("GET\r\n"), vec![Line::content("GET")]);
}

#[test]
fn content_line_with_trailing_cr() {
    assert_eq!(lex("GET\r"), vec![Line::content("GET")]);
}

#[test]
fn multiple_content_lines() {
    assert_eq!(
        lex("GET http://example.com\nContent-Type: application/json\n"),
        vec![
            Line::content("GET http://example.com"),
            Line::content("Content-Type: application/json"),
        ]
    );
}

#[test]
fn unicode_in_content_preserved() {
    assert_eq!(
        lex("GET /中文?q=你好\n"),
        vec![Line::content("GET /中文?q=你好")]
    );
}

#[test]
fn empty_line_classified_as_empty() {
    assert_eq!(lex("\n"), vec![Line::empty()]);
}

#[test]
fn blank_line_with_whitespace_is_empty() {
    // A line containing only whitespace is treated as empty (significant for
    // headers/body separation — see spec 3.2 `headers new-line [message-body]`).
    assert_eq!(lex("   \n"), vec![Line::empty()]);
}

#[test]
fn empty_line_between_two_content() {
    assert_eq!(
        lex("GET http://example.com\n\nContent-Type: x\n"),
        vec![
            Line::content("GET http://example.com"),
            Line::empty(),
            Line::content("Content-Type: x"),
        ]
    );
}

// --- P1-1: whitespace & continuation (spec 2.2 new-line-with-indent) ---

#[test]
fn indented_line_after_newline_is_indented() {
    // spec 3.2.1.3 example:
    //   http://example.com/
    //       api
    //       /get
    assert_eq!(
        lex("http://example.com/\n    api\n    /get\n"),
        vec![
            Line::content("http://example.com/"),
            Line::indented("    ", "api"),
            Line::indented("    ", "/get"),
        ]
    );
}

#[test]
fn tab_indent_counts_as_continuation() {
    assert_eq!(
        lex("POST /add\n\tContent-Type: x\n"),
        vec![
            Line::content("POST /add"),
            Line::indented("\t", "Content-Type: x")
        ]
    );
}

// --- P1-2: comments (spec 2.4) ---

#[test]
fn hash_comment_line() {
    assert_eq!(lex("# hello\n"), vec![Line::comment("hello")]);
}

#[test]
fn slash_slash_comment_line() {
    assert_eq!(lex("// hello\n"), vec![Line::comment("hello")]);
}

#[test]
fn comment_without_newline_at_eof() {
    assert_eq!(lex("# hello"), vec![Line::comment("hello")]);
}

#[test]
fn empty_hash_comment() {
    assert_eq!(lex("#\n"), vec![Line::comment("")]);
}

#[test]
fn hash_comment_trailing_whitespace_trimmed() {
    assert_eq!(lex("# hello   \n"), vec![Line::comment("hello")]);
}

// --- P7-1: `//` comments in the JetBrains examples (G1) ---

#[test]
fn slash_comment_without_space_is_still_a_comment() {
    // GET.http / POST.http use `//TIP …` with no space after the slashes.
    assert_eq!(
        lex("//TIP <p>Press <shortcut/></p>\n"),
        vec![Line::comment("TIP <p>Press <shortcut/></p>")],
    );
}

#[test]
fn separator_beats_slash_comment() {
    // `###` is classified before `//`, so `### //x` stays a separator whose
    // comment text happens to start with slashes.
    assert_eq!(
        lex("### //x\n"),
        vec![Line::separator(Some("//x".to_string()))],
    );
}

#[test]
fn indented_slash_line_is_indented_not_comment() {
    // Inside a `{% %}` script block the examples indent JS comments
    // (RequestWithScripts.http line 86). An indented `//` line is NOT a
    // document comment: it is collected verbatim as `Line::Indented` so the
    // script body keeps its `//` comment instead of losing the line.
    assert_eq!(
        lex("    // common script parts\n"),
        vec![Line::indented("    ", "// common script parts")],
    );
}

// --- P1-2: request separator (spec 2.5) ---

#[test]
fn separator_no_comment() {
    assert_eq!(lex("###\n"), vec![Line::separator(None)]);
}

#[test]
fn separator_with_comment() {
    assert_eq!(
        lex("### login request\n"),
        vec![Line::separator(Some("login request".to_string()))]
    );
}

#[test]
fn separator_without_newline_at_eof() {
    assert_eq!(lex("###"), vec![Line::separator(None)]);
}

#[test]
fn separator_comment_trimmed() {
    assert_eq!(
        lex("###   spaced   \n"),
        vec![Line::separator(Some("spaced".to_string()))]
    );
}

#[test]
fn hash_inside_content_line_not_a_comment() {
    // `#` not at start of line is just content. `##` not at start is content too.
    assert_eq!(
        lex("text # not a comment\n"),
        vec![Line::content("text # not a comment")]
    );
}

#[test]
fn four_hashes_is_separator_with_extra_hash_in_comment() {
    // `####` = `###` separator + `#` as comment text (after trim).
    assert_eq!(lex("####\n"), vec![Line::separator(Some("#".to_string()))]);
}

// --- Mixed: realistic fragment ---

#[test]
fn realistic_fragment_two_requests() {
    let src = "### first\nGET http://example.com\n### second\nPOST http://example.com/add\n";
    assert_eq!(
        lex(src),
        vec![
            Line::separator(Some("first".to_string())),
            Line::content("GET http://example.com"),
            Line::separator(Some("second".to_string())),
            Line::content("POST http://example.com/add"),
        ]
    );
}

#[test]
fn fragment_with_inline_comment_after_request() {
    let src = "### get user\nGET http://example.com/users/1\n# trailing comment\n";
    assert_eq!(
        lex(src),
        vec![
            Line::separator(Some("get user".to_string())),
            Line::content("GET http://example.com/users/1"),
            Line::comment("trailing comment"),
        ]
    );
}
