//! Parser for HTTP Request in Editor format (spec ch 3).
//!
//! Consumes the lexed [`Line`] stream (see [`crate::lexer`]) and produces a
//! [`RequestsFile`].
//!
//! Phase 1 covers: requests-file structure (3.1), request line (3.2.1),
//! request target (3.2.1.1~3.2.1.4), headers (3.2.2), message body
//! (3.2.3), multipart/form-data (3.2.3.1), response handler (3.2.4),
//! response ref (3.2.5).

use crate::error::{CoreError, Diagnostic, Result, Span};
use crate::lexer::{Lexer, Line};
use crate::model::{
    DocTag, HeaderField, MessageBody, MessagePartBody, Method, MultipartField, OutputRedirect,
    Request, RequestLine, RequestTarget, RequestsFile, ResponseHandler,
};

/// Parse a full `.http` file (spec 3.1).
///
/// Lenient: a malformed request is recorded as a [`Diagnostic`] on the
/// returned `RequestsFile` and skipped, so the rest of the file still parses.
/// A hard [`Err`] is only returned for catastrophic failures (currently none —
/// the lexer never errors and the parser always recovers).
pub fn parse_file(src: &str) -> Result<RequestsFile> {
    let lines: Vec<Line> = Lexer::new(src).collect();
    Parser::new(lines).parse()
}

struct Parser {
    lines: Vec<Line>,
    pos: usize,
}

impl Parser {
    fn new(lines: Vec<Line>) -> Self {
        Self { lines, pos: 0 }
    }

    fn parse(mut self) -> Result<RequestsFile> {
        let mut requests = Vec::new();
        let mut diagnostics: Vec<Diagnostic> = Vec::new();
        // Tracks the comment text of the most recently seen `###` separator,
        // so the next parsed request can pick it up as its name (JetBrains
        // HTTP Client convention). `None` means "no name".
        let mut pending_name: Option<String> = None;
        // Doc tags (`# @no-redirect`, …) accumulate from the comment lines
        // above a request and bind to the next request only.
        let mut pending_tags: Vec<DocTag> = Vec::new();
        // A pre-request script (`< {% … %}`) appears before the request line.
        let mut pending_pre: Option<ResponseHandler> = None;
        while self.pos < self.lines.len() {
            match &self.lines[self.pos] {
                Line::Separator { comment, .. } => {
                    pending_name = comment.clone().filter(|s| !s.trim().is_empty());
                    pending_tags.clear();
                    self.pos += 1;
                }
                Line::Comment { text, .. } => {
                    let text = text.clone();
                    pending_tags.extend(parse_doc_tags(&text));
                    self.pos += 1;
                }
                Line::Empty { .. } => {
                    self.pos += 1;
                }
                Line::Content { text, span } => {
                    let text = text.clone();
                    let span = *span;
                    // `< {% … %}` opens a pre-request script, not a request.
                    if pending_pre.is_none() {
                        if let Some(rest) = script_open_rest(&text, "<") {
                            self.pos += 1;
                            let rest = rest.to_string();
                            pending_pre = Some(ResponseHandler::Inline {
                                script: self.collect_script_body(&rest),
                            });
                            continue;
                        }
                    }
                    let tags = std::mem::take(&mut pending_tags);
                    let sep_name = pending_name.take();
                    // An explicit `@name` tag wins over the `### name` comment.
                    let name = tags
                        .iter()
                        .find_map(|t| match t {
                            DocTag::Name { value } => Some(value.clone()),
                            _ => None,
                        })
                        .or(sep_name);
                    let pre = pending_pre.take();
                    match self.parse_request(text, span, name, tags, pre) {
                        Ok(req) => requests.push(req),
                        Err(err) => {
                            diagnostics.push(err.into());
                            self.skip_to_next_separator();
                        }
                    }
                }
                Line::Indented { span, .. } => {
                    // Indented line at the top of a request with no preceding
                    // request line is malformed; record and skip.
                    diagnostics.push(Diagnostic {
                        kind: crate::error::ErrorKind::Parse,
                        span: *span,
                        message: "indented line outside a request".to_string(),
                    });
                    self.pos += 1;
                }
            }
        }
        Ok(RequestsFile {
            requests,
            diagnostics,
        })
    }

    /// Advance past everything until (and including) the next separator.
    fn skip_to_next_separator(&mut self) {
        while let Some(line) = self.lines.get(self.pos) {
            self.pos += 1;
            if matches!(line, Line::Separator { .. }) {
                break;
            }
        }
    }

    fn parse_request(
        &mut self,
        line_text: String,
        span: Span,
        name: Option<String>,
        tags: Vec<DocTag>,
        pre_request_script: Option<ResponseHandler>,
    ) -> Result<Request> {
        // Gather continuation lines (spec 2.2 new-line-with-indent) for the
        // request target (spec 3.2.1.3).
        self.pos += 1;
        let mut continuation: Vec<String> = Vec::new();
        while let Some(Line::Indented { text, .. }) = self.lines.get(self.pos) {
            continuation.push(text.trim().to_string());
            self.pos += 1;
        }

        let request_line = parse_request_line(&line_text, continuation, span)?;
        let headers = self.consume_headers();

        // Body kind is driven by Content-Type (multipart) or first body line.
        let content_type = find_header_ci(&headers, "content-type").map(|h| h.value.as_str());

        let body = if let Some(ct) = content_type {
            if ct.to_lowercase().contains("multipart/form-data") {
                let boundary = extract_multipart_boundary(ct).ok_or_else(|| {
                    CoreError::parse(
                        span,
                        "multipart/form-data Content-Type is missing a boundary",
                    )
                })?;
                self.parse_multipart_body(span, &boundary)?
            } else {
                self.parse_simple_body()
            }
        } else {
            self.parse_simple_body()
        };

        let (response_handler, response_ref, output_redirect) = self.parse_trailers();

        Ok(Request {
            line: request_line,
            headers,
            body,
            response_handler,
            response_ref,
            name,
            pre_request_script,
            output_redirect,
            tags,
        })
    }

    /// Consume header lines (Content + Indented continuation) until the next
    /// Empty / Separator / EOF. Returns parsed headers.
    fn consume_headers(&mut self) -> Vec<HeaderField> {
        let mut headers = Vec::new();
        while let Some(line) = self.lines.get(self.pos) {
            match line {
                Line::Separator { .. } | Line::Empty { .. } => break,
                Line::Comment { .. } => {
                    self.pos += 1;
                }
                Line::Content { text, .. } => {
                    // Stop at trailer lines (`> ` handler, `<> ` ref, `>>`
                    // redirect) and at body openers (`< file`) so a request
                    // with no blank line between its parts still reaches
                    // `parse_simple_body` / `parse_trailers` instead of being
                    // eaten as a (colon-less) header.
                    if text.starts_with('>') || text.starts_with('<') {
                        break;
                    }
                    self.pos += 1;
                    let mut value_continuation = Vec::new();
                    while let Some(Line::Indented { text: ct, .. }) = self.lines.get(self.pos) {
                        value_continuation.push(ct.clone());
                        self.pos += 1;
                    }
                    if let Some(h) = parse_header_field(text, value_continuation) {
                        headers.push(h);
                    }
                }
                Line::Indented { .. } => {
                    // Stray continuation outside a header value; skip.
                    self.pos += 1;
                }
            }
        }
        // Skip the terminating Empty line if present (it separates headers
        // from body — spec 3.2 `headers new-line [message-body]`).
        if let Some(Line::Empty { .. }) = self.lines.get(self.pos) {
            self.pos += 1;
        }
        headers
    }

    /// Parse a simple (non-multipart) message body (spec 3.2.3 `messages`).
    fn parse_simple_body(&mut self) -> Option<MessageBody> {
        // Peek past comments to find the first content line.
        let mut peek = self.pos;
        while let Some(Line::Comment { .. }) = self.lines.get(peek) {
            peek += 1;
        }
        if let Some(Line::Content { text, .. }) = self.lines.get(peek) {
            // spec 3.2.3 `input-file-ref: '<' required-whitespace file-path`.
            if let Some(rest) = text.strip_prefix('<') {
                if rest.starts_with(char::is_whitespace) {
                    let path = rest.trim_start().to_string();
                    // Advance past any comments we peeked over, plus the file-ref line.
                    self.pos = peek + 1;
                    return Some(MessageBody::FileRef { path });
                }
            }
        }

        // Inline body: collect Content / Indented / Empty lines until a
        // terminator (separator, response handler, response ref, EOF).
        let mut lines: Vec<String> = Vec::new();
        while let Some(line) = self.lines.get(self.pos) {
            match line {
                Line::Separator { .. } => break,
                Line::Content { text, .. } => {
                    // Body terminators per spec 3.2.3 message-line:
                    // `< ` (input-file-ref starts a new body kind), `<> `,
                    // `> ` (response handler), `>>` (output redirect),
                    // and `###` (separator, handled above).
                    if text.starts_with("<> ")
                        || text.starts_with("> ")
                        || text.starts_with(">>")
                        || text.starts_with('<')
                    {
                        break;
                    }
                    lines.push(text.clone());
                    self.pos += 1;
                }
                Line::Indented { indent, text, .. } => {
                    // For body mode, indented lines are NOT continuations —
                    // they are body lines with leading whitespace that must be
                    // preserved (spec 4.2.2 only trims the whole body, not
                    // each line).
                    let mut full = String::new();
                    full.push_str(indent);
                    full.push_str(text);
                    lines.push(full);
                    self.pos += 1;
                }
                Line::Empty { .. } => {
                    lines.push(String::new());
                    self.pos += 1;
                }
                Line::Comment { .. } => {
                    self.pos += 1;
                }
            }
        }

        if lines.is_empty() {
            return None;
        }
        // spec 4.2.2: in-place body whitespace around it is trimmed.
        let joined = lines.join("\n");
        let trimmed = joined.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(MessageBody::Inline {
                content: trimmed.to_string(),
            })
        }
    }

    /// Parse a multipart/form-data body (spec 3.2.3.1).
    fn parse_multipart_body(&mut self, span: Span, boundary: &str) -> Result<Option<MessageBody>> {
        let sep = format!("--{boundary}");
        let end_marker = format!("--{boundary}--");

        let mut parts: Vec<MultipartField> = Vec::new();
        let mut current: Option<PartBuilder> = None;

        while let Some(line) = self.lines.get(self.pos) {
            match line {
                Line::Separator { .. } => break,
                Line::Comment { .. } => {
                    self.pos += 1;
                }
                Line::Content { text, .. } => {
                    let text = text.clone();
                    self.pos += 1;
                    if text == end_marker {
                        if let Some(b) = current.take() {
                            parts.push(b.build());
                        }
                        return Ok(Some(MessageBody::Multipart {
                            boundary: boundary.to_string(),
                            parts,
                        }));
                    }
                    if text == sep {
                        if let Some(b) = current.take() {
                            parts.push(b.build());
                        }
                        current = Some(PartBuilder::new());
                    } else if let Some(builder) = current.as_mut() {
                        builder.add_line(&text);
                    }
                    // Lines before the first `--boundary` are stray — skip.
                }
                Line::Indented { indent, text, .. } => {
                    let mut full = String::new();
                    full.push_str(indent);
                    full.push_str(text);
                    self.pos += 1;
                    if let Some(builder) = current.as_mut() {
                        builder.add_line(&full);
                    }
                }
                Line::Empty { .. } => {
                    self.pos += 1;
                    if let Some(builder) = current.as_mut() {
                        builder.add_empty();
                    }
                }
            }
        }

        if parts.is_empty() && current.is_none() {
            // No boundary encountered — not actually multipart; fall back to None.
            return Ok(None);
        }
        if let Some(b) = current.take() {
            parts.push(b.build());
        }
        if parts.is_empty() {
            Err(CoreError::parse(
                span,
                "multipart body declared but no parts found",
            ))
        } else {
            Ok(Some(MessageBody::Multipart {
                boundary: boundary.to_string(),
                parts,
            }))
        }
    }

    /// Parse the trailer lines that may follow a message body, in any order:
    /// `> {% … %}` / `> file` (response handler, spec 3.2.4), `<> file`
    /// (response ref, spec 3.2.5), `>> file` / `>>! file` (output redirect).
    fn parse_trailers(
        &mut self,
    ) -> (
        Option<ResponseHandler>,
        Option<String>,
        Option<OutputRedirect>,
    ) {
        let mut handler = None;
        let mut reference = None;
        let mut redirect = None;
        loop {
            // Comments interleaved with trailers are insignificant (`//TIP …`).
            while matches!(self.lines.get(self.pos), Some(Line::Comment { .. })) {
                self.pos += 1;
            }
            let Some(Line::Content { text, .. }) = self.lines.get(self.pos) else {
                break;
            };
            let text = text.clone();

            if handler.is_none() {
                if let Some(rest) = script_open_rest(&text, ">") {
                    self.pos += 1;
                    let rest = rest.to_string();
                    handler = Some(ResponseHandler::Inline {
                        script: self.collect_script_body(&rest),
                    });
                    continue;
                }
                if let Some(rest) = text.strip_prefix("> ") {
                    self.pos += 1;
                    handler = Some(ResponseHandler::FileRef {
                        path: rest.trim().to_string(),
                    });
                    continue;
                }
            }
            if redirect.is_none() {
                if let Some(r) = parse_output_redirect(&text) {
                    self.pos += 1;
                    redirect = Some(r);
                    continue;
                }
            }
            if reference.is_none() {
                if let Some(rest) = text.strip_prefix("<> ") {
                    self.pos += 1;
                    reference = Some(rest.trim().to_string());
                    continue;
                }
            }
            break;
        }
        (handler, reference, redirect)
    }

    /// Collect the body of a `{% … %}` script block. `first` is the text that
    /// followed `{%` on the opening line; the current position is the line
    /// after it. Consumes lines up to and including the one ending in `%}`.
    ///
    /// Handles both the single-line form (`> {% script %}`) and the multiline
    /// form used throughout the JetBrains examples.
    fn collect_script_body(&mut self, first: &str) -> String {
        let head = first.trim_end();
        // Single-line form.
        if let Some(idx) = head.rfind("%}") {
            return head[..idx].trim().to_string();
        }
        let mut out: Vec<String> = Vec::new();
        if !head.is_empty() {
            out.push(head.to_string());
        }
        while let Some(line) = self.lines.get(self.pos) {
            let raw = match line {
                Line::Content { text, .. } => text.clone(),
                Line::Indented { indent, text, .. } => format!("{indent}{text}"),
                Line::Empty { .. } => String::new(),
                // The lexer already stripped the marker; `//` restores a valid
                // JS comment for either `//` or `#` originals.
                Line::Comment { text, .. } => format!("//{text}"),
                Line::Separator { .. } => break,
            };
            self.pos += 1;
            let trimmed = raw.trim_end();
            if let Some(kept) = trimmed.strip_suffix("%}") {
                let kept = kept.trim_end();
                if !kept.is_empty() {
                    out.push(kept.to_string());
                }
                break;
            }
            out.push(raw);
        }
        out.join("\n").trim().to_string()
    }
}

/// If `text` opens a `{% … %}` script block after `marker` (`"<"` for a
/// pre-request script, `">"` for a response handler), return the text that
/// follows `{%`. `>>` (output redirect) never matches the `">"` marker.
fn script_open_rest<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let rest = text.strip_prefix(marker)?.trim_start();
    if marker == ">" && rest.starts_with('>') {
        return None;
    }
    rest.strip_prefix("{%")
}

/// Parse a `>> path` / `>>! path` output-redirect line.
fn parse_output_redirect(text: &str) -> Option<OutputRedirect> {
    let rest = text.strip_prefix(">>")?;
    let (force, rest) = match rest.strip_prefix('!') {
        Some(r) => (true, r),
        None => (false, rest),
    };
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let path = rest.trim().to_string();
    if path.is_empty() {
        return None;
    }
    Some(OutputRedirect { path, force })
}

/// Recognised documentation tags (JetBrains HTTP Client extension).
const TAG_NAMES: &[&str] = &[
    "no-redirect",
    "no-cookie-jar",
    "no-auto-encoding",
    "no-log",
    "connection-timeout",
    "timeout",
    "name",
];

/// Harvest `@tag [value]` occurrences from a comment's text.
///
/// Unknown `@words` are ignored so ordinary prose comments stay inert.
/// `connection-timeout` is listed before `timeout` so the longer name wins.
fn parse_doc_tags(text: &str) -> Vec<DocTag> {
    let mut tags = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '@' {
            i += 1;
            continue;
        }
        let name_start = i + 1;
        let mut name_end = name_start;
        while name_end < chars.len()
            && (chars[name_end].is_ascii_alphanumeric() || chars[name_end] == '-')
        {
            name_end += 1;
        }
        let name: String = chars[name_start..name_end].iter().collect();
        if !TAG_NAMES.contains(&name.as_str()) {
            i = name_end.max(i + 1);
            continue;
        }
        // Value runs to the next `@` or end of line.
        let mut value_end = name_end;
        while value_end < chars.len() && chars[value_end] != '@' {
            value_end += 1;
        }
        let value: String = chars[name_end..value_end].iter().collect();
        let value = value.trim();
        let tag = match name.as_str() {
            "no-redirect" => DocTag::NoRedirect,
            "no-cookie-jar" => DocTag::NoCookieJar,
            "no-auto-encoding" => DocTag::NoAutoEncoding,
            "no-log" => DocTag::NoLog,
            "timeout" | "connection-timeout" => {
                let millis = value.split_whitespace().next().unwrap_or("").parse::<u64>();
                match millis {
                    Ok(m) if name == "timeout" => DocTag::Timeout { millis: m },
                    Ok(m) => DocTag::ConnectionTimeout { millis: m },
                    // A numeric tag with no parsable value is not a tag at all.
                    Err(_) => {
                        i = value_end.max(name_end);
                        continue;
                    }
                }
            }
            _ => DocTag::Name {
                value: value.to_string(),
            },
        };
        tags.push(tag);
        i = value_end.max(name_end);
    }
    tags
}

/// Build a multipart field incrementally from raw body lines.
struct PartBuilder {
    headers: Vec<HeaderField>,
    in_headers: bool,
    body_lines: Vec<String>,
}

impl PartBuilder {
    fn new() -> Self {
        Self {
            headers: Vec::new(),
            in_headers: true,
            body_lines: Vec::new(),
        }
    }

    fn add_line(&mut self, text: &str) {
        if self.in_headers {
            if text.is_empty() {
                self.in_headers = false;
            } else if let Some(h) = parse_header_field(text, Vec::new()) {
                self.headers.push(h);
            }
        } else {
            self.body_lines.push(text.to_string());
        }
    }

    /// An empty line within the body section is preserved as a blank line.
    fn add_empty(&mut self) {
        if !self.in_headers {
            self.body_lines.push(String::new());
        } else {
            self.in_headers = false;
        }
    }

    fn build(self) -> MultipartField {
        let body = if self.body_lines.len() == 1 {
            let line = &self.body_lines[0];
            if let Some(rest) = line.strip_prefix('<') {
                if rest.starts_with(char::is_whitespace) {
                    let path = rest.trim_start().to_string();
                    return MultipartField {
                        headers: self.headers,
                        body: MessagePartBody::FileRef { path },
                    };
                }
            }
            MessagePartBody::Inline {
                content: self.body_lines.join("\n"),
            }
        } else if self.body_lines.is_empty() {
            MessagePartBody::Inline {
                content: String::new(),
            }
        } else {
            MessagePartBody::Inline {
                content: self.body_lines.join("\n"),
            }
        };
        MultipartField {
            headers: self.headers,
            body,
        }
    }
}

fn parse_request_line(text: &str, continuation: Vec<String>, span: Span) -> Result<RequestLine> {
    // spec 3.2.6 env vars may contain whitespace (`{{ element-id }}`), so a
    // plain `split_whitespace` would over-split. Use a `{{`-aware splitter.
    let tokens = split_request_line_tokens(text);
    if tokens.is_empty() {
        return Err(CoreError::parse(span, "empty request line"));
    }

    let (method, target_start) = match Method::from_str(tokens[0].as_str()) {
        Some(m) => (m, 1),
        None => (Method::Get, 0),
    };

    let remaining = &tokens[target_start..];
    if remaining.is_empty() {
        return Err(CoreError::parse(span, "missing request target"));
    }

    let (target_token, http_version) = match remaining.len() {
        1 => (remaining[0].clone(), None),
        2 if is_http_version(&remaining[1]) => (remaining[0].clone(), Some(remaining[1].clone())),
        _ => {
            return Err(CoreError::parse(
                span,
                "invalid request line: too many tokens",
            ))
        }
    };

    let mut target_text = target_token;
    for seg in continuation {
        target_text.push_str(&seg);
    }

    let target = parse_target(&target_text, span)?;
    Ok(RequestLine {
        method,
        target,
        http_version,
    })
}

/// Split a request line into whitespace-separated tokens, treating
/// `{{ ... }}` environment-variable references as opaque (whitespace inside
/// them does not split — spec 3.2.6).
fn split_request_line_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_env = false;
    let mut env_depth = 0usize;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_env {
            current.push(ch);
            if ch == '{' && matches!(chars.peek(), Some('{')) {
                env_depth += 1;
                current.push(chars.next().unwrap());
            } else if ch == '}' && matches!(chars.peek(), Some('}')) {
                env_depth -= 1;
                current.push(chars.next().unwrap());
                if env_depth == 0 {
                    in_env = false;
                }
            }
            continue;
        }
        if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }
        if ch == '{' && matches!(chars.peek(), Some('{')) {
            in_env = true;
            env_depth = 1;
            current.push(ch);
            current.push(chars.next().unwrap());
            continue;
        }
        current.push(ch);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn is_http_version(s: &str) -> bool {
    // spec 3.2.1: `HTTP/` (digit)+ `.` (digit)+
    // JetBrains also accepts the bare minor-less form (`HTTP/2`), which the
    // official examples use, so `HTTP/` (digit)+ is accepted as well.
    let Some(rest) = s.strip_prefix("HTTP/") else {
        return false;
    };
    if rest.is_empty() {
        return false;
    }
    let all_digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    match rest.split_once('.') {
        Some((major, minor)) => all_digits(major) && all_digits(minor),
        None => all_digits(rest),
    }
}

fn parse_target(s: &str, span: Span) -> Result<RequestTarget> {
    if s.is_empty() {
        return Err(CoreError::parse(span, "empty request target"));
    }
    if s == "*" {
        return Ok(RequestTarget::Asterisk);
    }
    // Scheme-relative absolute-form (`//host/...`) defaults scheme to `http`.
    if s.starts_with("//") {
        return parse_absolute_form(s, span);
    }
    if let Some(rest) = s.strip_prefix('/') {
        return parse_origin_form(rest, span);
    }
    parse_absolute_form(s, span)
}

/// `origin-form` = `/` absolute-path [`?` query] [`#` fragment] (spec 3.2.1.1).
/// Caller passes `rest` = target with the leading `/` stripped.
fn parse_origin_form(rest: &str, _span: Span) -> Result<RequestTarget> {
    let full = format!("/{rest}");
    let path_end = full.find(['?', '#']).unwrap_or(full.len());
    let path = full[..path_end].to_string();
    let after_path = &full[path_end..];
    let (query, fragment) = split_query_fragment(after_path);
    Ok(RequestTarget::Origin {
        path,
        query,
        fragment,
    })
}

/// `absolute-form` = `[scheme '://'] hier-part ['?' query] ['#' fragment] (spec 3.2.1.1).
fn parse_absolute_form(s: &str, span: Span) -> Result<RequestTarget> {
    let (scheme, hier) = match s.find("://") {
        Some(idx) => (Some(s[..idx].to_string()), &s[idx + 3..]),
        None => {
            if let Some(rest) = s.strip_prefix("//") {
                (Some("http".to_string()), rest)
            } else {
                (Some("http".to_string()), s)
            }
        }
    };

    if hier.is_empty() {
        return Err(CoreError::parse(span, "absolute-form missing authority"));
    }

    let auth_end = hier.find(['/', '?', '#']).unwrap_or(hier.len());
    let authority = hier[..auth_end].to_string();
    let after_auth = &hier[auth_end..];

    let path = after_auth.strip_prefix('/').map(|_| {
        let end = after_auth.find(['?', '#']).unwrap_or(after_auth.len());
        after_auth[..end].to_string()
    });

    let after_path = &after_auth[path.as_ref().map_or(0, |p| p.len())..];
    let (query, fragment) = split_query_fragment(after_path);

    Ok(RequestTarget::Absolute {
        scheme,
        authority,
        path,
        query,
        fragment,
    })
}

/// Split a trailing `?query` / `#fragment` section into its parts.
fn split_query_fragment(s: &str) -> (Option<String>, Option<String>) {
    if s.is_empty() {
        return (None, None);
    }
    let q_start = s.find('?');
    let f_start = s.find('#');
    let query = q_start.map(|i| {
        let end = f_start.filter(|&f| f > i).unwrap_or(s.len());
        s[i + 1..end].to_string()
    });
    let fragment = f_start.map(|i| s[i + 1..].to_string());
    (query, fragment)
}

fn parse_header_field(text: &str, continuation: Vec<String>) -> Option<HeaderField> {
    let colon = text.find(':')?;
    let name = text[..colon].to_string();
    let mut value = text[colon + 1..].trim().to_string();
    for c in continuation {
        if !value.is_empty() {
            value.push(' ');
        }
        value.push_str(c.trim());
    }
    Some(HeaderField { name, value })
}

fn find_header_ci<'a>(headers: &'a [HeaderField], name: &str) -> Option<&'a HeaderField> {
    headers.iter().find(|h| h.name.eq_ignore_ascii_case(name))
}

/// Extract the multipart boundary from a Content-Type value (spec 3.2.3.1).
fn extract_multipart_boundary(content_type: &str) -> Option<String> {
    let lower = content_type.to_lowercase();
    if !lower.contains("multipart/form-data") {
        return None;
    }
    let idx = lower.find("boundary=")?;
    let after = &content_type[idx + "boundary=".len()..];
    let boundary = if let Some(rest) = after.strip_prefix('"') {
        let end = rest.find('"')?;
        rest[..end].to_string()
    } else {
        let end = after.find(';').unwrap_or(after.len());
        after[..end].trim().to_string()
    };
    if boundary.is_empty() {
        None
    } else {
        Some(boundary)
    }
}

impl Method {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "GET" => Some(Self::Get),
            "HEAD" => Some(Self::Head),
            "POST" => Some(Self::Post),
            "PUT" => Some(Self::Put),
            "DELETE" => Some(Self::Delete),
            "CONNECT" => Some(Self::Connect),
            "PATCH" => Some(Self::Patch),
            "OPTIONS" => Some(Self::Options),
            "TRACE" => Some(Self::Trace),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_version_check() {
        assert!(is_http_version("HTTP/1.1"));
        assert!(is_http_version("HTTP/2.0"));
        // Bare minor-less form, as used by the official examples (`… HTTP/2`).
        assert!(is_http_version("HTTP/2"));
        assert!(is_http_version("HTTP/1"));
        assert!(!is_http_version("HTTP/x.1"));
        assert!(!is_http_version("HTTP/x"));
        assert!(!is_http_version("HTTP/"));
        assert!(!is_http_version("HTTP1.1"));
        assert!(!is_http_version("http://example.com"));
    }

    #[test]
    fn target_asterisk() {
        assert_eq!(
            parse_target("*", Span::new(0, 0)).unwrap(),
            RequestTarget::Asterisk
        );
    }

    #[test]
    fn target_origin_form_basics() {
        let t = parse_target("/api/get?id=42#frag", Span::new(0, 0)).unwrap();
        match t {
            RequestTarget::Origin {
                path,
                query,
                fragment,
            } => {
                assert_eq!(path, "/api/get");
                assert_eq!(query.as_deref(), Some("id=42"));
                assert_eq!(fragment.as_deref(), Some("frag"));
            }
            other => panic!("expected Origin, got {other:?}"),
        }
    }

    #[test]
    fn multipart_boundary_extraction() {
        assert_eq!(
            extract_multipart_boundary("multipart/form-data; boundary=abcd"),
            Some("abcd".to_string())
        );
        assert_eq!(
            extract_multipart_boundary("multipart/form-data; boundary=\"quoted\""),
            Some("quoted".to_string())
        );
        assert_eq!(extract_multipart_boundary("multipart/form-data"), None);
        assert_eq!(extract_multipart_boundary("application/json"), None);
    }

    #[test]
    fn handler_inline_vs_file() {
        let inline = parse_file("GET https://x.test/a\n> {% client.global.set(\"x\", 1); %}\n").unwrap();
        assert!(matches!(
            inline.requests[0].response_handler,
            Some(ResponseHandler::Inline { .. })
        ));

        let file = parse_file("GET https://x.test/a\n> ./handler.js\n").unwrap();
        assert!(matches!(
            file.requests[0].response_handler,
            Some(ResponseHandler::FileRef { .. })
        ));
    }
}
