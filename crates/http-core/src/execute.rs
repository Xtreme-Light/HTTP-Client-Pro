//! Execution: turn an AST [`Request`] into a [`PreparedRequest`] ready for
//! dispatch (spec chapter 4.1~4.3).
//!
//! Pipeline: substitute env vars (4.4) → encode path/query (4.1.2) → trim
//! multiline path segments (4.2.1) → assemble URL → encode body (4.1.3) →
//! trim in-place body (4.2.2). Dispatch against a real HTTP client lives
//! in [`crate::dispatch`].

use crate::env::{substitute, substitute_json, Environment, VarValue};
use crate::error::{CoreError, ErrorKind, Result, Span};
use crate::jsonpath;
use crate::model::{
    DocTag, HeaderField, MessageBody, MessagePartBody, Method, MultipartField, Request,
    RequestTarget,
};
use std::path::{Path, PathBuf};

/// A request ready to be sent over the wire.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedRequest {
    pub method: Method,
    pub url: String,
    pub http_version: Option<String>,
    pub headers: Vec<HeaderField>,
    pub body: Option<Vec<u8>>,
    /// Multipart boundary (if multipart), for the dispatcher to use when
    /// assembling the body — included here for tests / debugging.
    pub multipart_boundary: Option<String>,
}

/// Percent-encode a path string (spec 4.1.2):
/// - Non-ASCII chars are percent-encoded (UTF-8 bytes).
/// - Already-encoded `%XX` sequences are preserved (not double-encoded).
/// - All ASCII chars are passed through unchanged (the spec only requires
///   encoding non-ASCII; sending literal ASCII `%`, `=`, `&`, space is the
///   user's responsibility — they can write `%20` for a space).
pub fn encode_path(s: &str) -> String {
    encode_uri_component(s)
}

/// Percent-encode a query string (spec 4.1.2). Same rules as path.
pub fn encode_query(s: &str) -> String {
    encode_uri_component(s)
}

fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' {
            // Preserve a valid `%XX` already-encoded sequence verbatim.
            if i + 2 < bytes.len()
                && bytes[i + 1].is_ascii_hexdigit()
                && bytes[i + 2].is_ascii_hexdigit()
            {
                out.push(bytes[i] as char);
                out.push(bytes[i + 1] as char);
                out.push(bytes[i + 2] as char);
                i += 3;
                continue;
            }
            // Invalid `%` not followed by two hex digits: spec is silent;
            // we pass the `%` through unchanged (it is ASCII).
            out.push('%');
            i += 1;
            continue;
        }
        if b < 0x80 {
            out.push(b as char);
            i += 1;
        } else {
            // Non-ASCII: encode the UTF-8 byte sequence.
            let ch = s[i..].chars().next().expect("valid utf8");
            for bb in ch.to_string().as_bytes() {
                push_pct(&mut out, *bb);
            }
            i += ch.len_utf8();
        }
    }
    out
}

fn push_pct(out: &mut String, b: u8) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    out.push('%');
    out.push(HEX[(b >> 4) as usize] as char);
    out.push(HEX[(b & 0x0f) as usize] as char);
}

/// Build a [`PreparedRequest`] from an AST [`Request`] and an [`Environment`].
///
/// File references (`< ./file.txt` and multipart `< file` parts) are left
/// unresolved — `body` is `None` for an unresolved `FileRef` body, and an
/// unresolved multipart `FileRef` part contributes zero body bytes. Use
/// [`prepare_with`] with a [`FileResolver`] (e.g. [`DiskResolver`]) to inline
/// file contents at prepare time.
pub fn prepare(req: &Request, env: &Environment) -> Result<PreparedRequest> {
    prepare_with(req, env, &NoResolver)
}

/// Like [`prepare`], but uses `resolver` to read file references inline so the
/// returned [`PreparedRequest`] is fully self-contained and ready for
/// dispatch over the wire.
pub fn prepare_with(
    req: &Request,
    env: &Environment,
    resolver: &dyn FileResolver,
) -> Result<PreparedRequest> {
    let method = req.line.method.clone();
    let http_version = req.line.http_version.clone();

    // Headers: substitute env vars in each value (and name, defensively).
    let mut headers: Vec<HeaderField> = req
        .headers
        .iter()
        .map(|h| HeaderField {
            name: substitute(&h.name, env),
            value: substitute(&h.value, env),
        })
        .collect();

    // URL: substitute, then encode path/query, then resolve origin/asterisk.
    // `@no-auto-encoding` (plan §7.6 G13) sends the path/query verbatim.
    let no_auto_encode = req.has_tag(&DocTag::NoAutoEncoding);
    let url = build_url(&req.line.target, env, &headers, no_auto_encode)?;

    // Body: substitute + trim/encode per spec 4.2.2 / 4.1.3, resolve file refs.
    let (body_bytes, multipart_boundary) = build_body(&req.body, env, &headers, resolver)?;

    let _ = &mut headers; // headers are already final
    Ok(PreparedRequest {
        method,
        url,
        http_version,
        headers,
        body: body_bytes,
        multipart_boundary,
    })
}

/// Resolves `< file` references encountered while preparing a request body.
///
/// The path is passed verbatim from the AST (e.g. `"./input.txt"`); the
/// resolver decides how to interpret it (relative to a base dir, absolute,
/// virtual, etc.).
pub trait FileResolver {
    fn read(&self, path: &str) -> Result<Vec<u8>>;
}

/// No-op resolver used by [`prepare`] — file refs contribute zero body bytes.
pub struct NoResolver;

impl FileResolver for NoResolver {
    fn read(&self, _path: &str) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }
}

/// Reads file references from disk, resolving relative paths against `base`.
///
/// `base` may be empty / `.` to resolve against the current working directory.
#[derive(Debug, Clone)]
pub struct DiskResolver {
    base: PathBuf,
}

impl DiskResolver {
    pub fn new(base: impl AsRef<Path>) -> Self {
        Self {
            base: base.as_ref().to_path_buf(),
        }
    }
}

impl Default for DiskResolver {
    fn default() -> Self {
        Self::new("")
    }
}

impl FileResolver for DiskResolver {
    fn read(&self, path: &str) -> Result<Vec<u8>> {
        let p = std::path::PathBuf::from(path);
        let resolved = if p.is_absolute() {
            p
        } else {
            self.base.join(p)
        };
        std::fs::read(&resolved).map_err(|e| {
            CoreError::new(
                ErrorKind::Io,
                Span::new(0, 0),
                format!("failed to read file ref `{}`: {e}", resolved.display()),
            )
        })
    }
}

fn build_url(
    target: &RequestTarget,
    env: &Environment,
    headers: &[HeaderField],
    no_auto_encode: bool,
) -> Result<String> {
    // When `@no-auto-encoding` is set, path/query are substituted but sent
    // verbatim (no percent-encoding of non-ASCII / special chars).
    let enc_path = |s: String| {
        if no_auto_encode {
            s
        } else {
            encode_path(&s)
        }
    };
    let enc_query = |s: String| {
        if no_auto_encode {
            s
        } else {
            encode_query(&s)
        }
    };
    let host_header = headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("Host"))
        .map(|h| h.value.as_str());

    let span = Span::new(0, 0);
    match target {
        RequestTarget::Asterisk => {
            // Asterisk requires a Host to be addressable.
            let host = host_header.ok_or_else(|| {
                CoreError::new(
                    crate::error::ErrorKind::Execute,
                    span,
                    "asterisk-form requires a Host header",
                )
            })?;
            let host = substitute(host, env);
            Ok(format!("http://{host}*"))
        }
        RequestTarget::Origin {
            path,
            query,
            fragment,
        } => {
            let host = host_header.ok_or_else(|| {
                CoreError::new(
                    crate::error::ErrorKind::Execute,
                    span,
                    "origin-form requires a Host header to be executable",
                )
            })?;
            let host = substitute(host, env);
            let path_enc = enc_path(substitute(path, env));
            let mut url = format!("http://{host}{path_enc}");
            if let Some(q) = query {
                url.push('?');
                url.push_str(&enc_query(substitute(q, env)));
            }
            if let Some(f) = fragment {
                url.push('#');
                url.push_str(&substitute(f, env));
            }
            Ok(url)
        }
        RequestTarget::Absolute {
            scheme,
            authority,
            path,
            query,
            fragment,
        } => {
            let scheme = scheme
                .as_deref()
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_else(|| "http".to_string());
            let auth = substitute(authority, env);
            // The JetBrains convention is to keep `scheme://host[:port]` in a
            // single variable (`GET {{host}}/get`) so environments can switch
            // schemes. A substituted authority that carries its own scheme
            // therefore wins over the one parsed from the request line.
            let (scheme, auth) = match auth.split_once("://") {
                Some((s, rest)) if is_scheme(s) => (s.to_ascii_lowercase(), rest.to_string()),
                _ => (scheme, auth),
            };
            let mut url = format!("{scheme}://{auth}");
            if let Some(p) = path {
                url.push_str(&enc_path(substitute(p, env)));
            }
            if let Some(q) = query {
                url.push('?');
                url.push_str(&enc_query(substitute(q, env)));
            }
            if let Some(f) = fragment {
                url.push('#');
                url.push_str(&substitute(f, env));
            }
            Ok(url)
        }
    }
}

/// RFC 3986 `scheme = ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )`. Used to tell
/// a real scheme apart from an authority that merely contains `://`.
fn is_scheme(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => chars.all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')
        }),
        _ => false,
    }
}

fn build_body(
    body: &Option<MessageBody>,
    env: &Environment,
    headers: &[HeaderField],
    resolver: &dyn FileResolver,
) -> Result<(Option<Vec<u8>>, Option<String>)> {
    let content_type = find_header_ci(headers, "content-type")
        .map(|h| h.value.to_ascii_lowercase())
        .unwrap_or_default();
    let is_json = content_type.contains("json");
    let is_urlencoded = content_type.contains("application/x-www-form-urlencoded");

    match body {
        None => Ok((None, None)),
        Some(MessageBody::Inline { content }) => {
            // spec 4.2.2: in-place body whitespace around it is trimmed.
            if is_urlencoded {
                // plan §7.5 G12: format `k = v &` lines into a proper
                // application/x-www-form-urlencoded body, restoring `%`-escaped
                // separators first.
                let substituted = substitute(content, env);
                let encoded = format_urlencoded(&substituted);
                return Ok((Some(encoded.into_bytes()), None));
            }
            let substituted = if is_json {
                // plan §7.3 G24: auto-quote string-valued dynamic variables
                // that sit in an unquoted JSON position.
                substitute_json(content, env)
            } else {
                substitute(content, env)
            };
            let trimmed = substituted.trim();
            if trimmed.is_empty() {
                Ok((Some(Vec::new()), None))
            } else {
                // spec 4.1.3: default encoding is UTF-8.
                Ok((Some(trimmed.as_bytes().to_vec()), None))
            }
        }
        Some(MessageBody::FileRef { path }) => {
            // spec 4.2.2: file body is NOT trimmed — full file content sent
            // verbatim. Resolution is delegated to `resolver`.
            let bytes = resolver.read(path)?;
            Ok((Some(bytes), None))
        }
        Some(MessageBody::Multipart { boundary, parts }) => {
            let bytes = build_multipart_body(boundary, parts, env, resolver)?;
            Ok((Some(bytes), Some(boundary.clone())))
        }
    }
}

/// Format an `application/x-www-form-urlencoded` body (plan §7.5 G12).
///
/// The JetBrains in-editor form lets the user write readable multi-line
/// `key = value &` pairs with `%`-escaped literal separators (`%+`, `%=`,
/// `%&`, `%%`). Steps:
/// 1. Join continuation lines (a trailing `&` or a bare newline both separate
///    pairs).
/// 2. Restore `%`-escaped separators to their literal characters *within a
///    value* (handled by splitting on real `&`/`=` first, then unescaping).
/// 3. Split on real `&`, then on the first real `=`, trim, and percent-encode
///    each key/value with the form-urlencoded rules (space → `+`).
pub fn format_urlencoded(raw: &str) -> String {
    // Normalize newlines into `&` separators: a line ending in `&` already
    // separates; a newline without `&` also separates pairs. Collapse the
    // whole body into a single `&`-delimited string.
    let mut joined = String::new();
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if !joined.is_empty() && !joined.ends_with('&') {
            joined.push('&');
        }
        joined.push_str(t);
    }

    // Split on *unescaped* `&` / `=` only: `%&` and `%=` are literal value
    // characters, so the escape must be honoured while scanning.
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut key = String::new();
    let mut value = String::new();
    let mut in_value = false;
    let mut started = false;
    let chars: Vec<char> = joined.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '%' && i + 1 < chars.len() && matches!(chars[i + 1], '&' | '=' | '+' | '%') {
            // Escaped separator: keep the two chars verbatim for unescaping.
            if in_value {
                value.push(c);
                value.push(chars[i + 1]);
            } else {
                key.push(c);
                key.push(chars[i + 1]);
            }
            i += 2;
            continue;
        }
        match c {
            '&' => {
                if started {
                    pairs.push((key.clone(), value.clone()));
                }
                key.clear();
                value.clear();
                in_value = false;
                started = false;
            }
            '=' if !in_value => {
                in_value = true;
                started = true;
            }
            _ => {
                started = true;
                if in_value {
                    value.push(c);
                } else {
                    key.push(c);
                }
            }
        }
        i += 1;
    }
    if started || !key.trim().is_empty() {
        pairs.push((key, value));
    }

    for (k, v) in pairs {
        let key = unescape_separators(k.trim());
        let value = unescape_separators(v.trim());
        if key.is_empty() && value.is_empty() {
            continue;
        }
        serializer.append_pair(&key, &value);
    }
    serializer.finish()
}

/// Restore JetBrains `%`-escaped separators (`%+`→`+`, `%=`→`=`, `%&`→`&`,
/// `%%`→`%`) inside a form field so they become literal value characters.
fn unescape_separators(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.peek() {
                Some('+') => {
                    out.push('+');
                    chars.next();
                }
                Some('=') => {
                    out.push('=');
                    chars.next();
                }
                Some('&') => {
                    out.push('&');
                    chars.next();
                }
                Some('%') => {
                    out.push('%');
                    chars.next();
                }
                _ => out.push('%'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn find_header_ci<'a>(headers: &'a [HeaderField], name: &str) -> Option<&'a HeaderField> {
    headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
}

fn build_multipart_body(
    boundary: &str,
    parts: &[MultipartField],
    env: &Environment,
    resolver: &dyn FileResolver,
) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for part in parts {
        // `--boundary`
        out.extend_from_slice(b"--");
        out.extend_from_slice(boundary.as_bytes());
        out.extend_from_slice(b"\r\n");
        // Headers
        for h in &part.headers {
            let name = substitute(&h.name, env);
            let value = substitute(&h.value, env);
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(b": ");
            out.extend_from_slice(value.as_bytes());
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"\r\n");
        // Body
        match &part.body {
            MessagePartBody::Inline { content } => {
                let substituted = substitute(content, env);
                out.extend_from_slice(substituted.as_bytes());
            }
            MessagePartBody::FileRef { path } => {
                let bytes = resolver.read(path)?;
                out.extend_from_slice(&bytes);
            }
        }
        out.extend_from_slice(b"\r\n");
    }
    // Closing `--boundary--`
    out.extend_from_slice(b"--");
    out.extend_from_slice(boundary.as_bytes());
    out.extend_from_slice(b"--\r\n");
    Ok(out)
}

// ---------------------------------------------------------------------------
// JSONPath loop expansion (plan §7.4 G10)
// ---------------------------------------------------------------------------

/// One loop iteration of a request. `bindings` maps each JSONPath template
/// name (e.g. `$.clients..id`) to the value bound for this iteration, in order
/// of first appearance; `template_values` is the same values as JSON, indexed
/// by template order (backs `request.templateValue(i)`).
#[derive(Debug, Clone, PartialEq)]
pub struct Iteration {
    pub index: usize,
    pub bindings: Vec<(String, VarValue)>,
    pub template_values: Vec<serde_json::Value>,
}

/// Gather every `{{ … }}` reference in a request's substitutable text (URL
/// target, headers, inline body), preserving first-appearance order and
/// de-duplicating. Only JSONPath templates (`$.…`) matter for loops, but the
/// scan returns all names so callers can filter.
fn request_references(req: &Request) -> Vec<String> {
    let mut texts: Vec<String> = Vec::new();
    match &req.line.target {
        RequestTarget::Origin {
            path,
            query,
            fragment,
        } => {
            texts.push(path.clone());
            if let Some(q) = query {
                texts.push(q.clone());
            }
            if let Some(f) = fragment {
                texts.push(f.clone());
            }
        }
        RequestTarget::Absolute {
            authority,
            path,
            query,
            fragment,
            ..
        } => {
            texts.push(authority.clone());
            if let Some(p) = path {
                texts.push(p.clone());
            }
            if let Some(q) = query {
                texts.push(q.clone());
            }
            if let Some(f) = fragment {
                texts.push(f.clone());
            }
        }
        RequestTarget::Asterisk => {}
    }
    for h in &req.headers {
        texts.push(h.name.clone());
        texts.push(h.value.clone());
    }
    if let Some(MessageBody::Inline { content }) = &req.body {
        texts.push(content.clone());
    }

    let mut seen = Vec::new();
    for text in &texts {
        for name in crate::env::collect_references(text) {
            if !seen.contains(&name) {
                seen.push(name);
            }
        }
    }
    seen
}

/// Expand a request into its loop iterations (plan §7.4 G10).
///
/// 1. Scan URL/headers/body for `{{$.…}}` JSONPath templates (first-appearance
///    order, de-duplicated).
/// 2. Evaluate each against the environment's [`Environment::variables_root`].
/// 3. Iteration count = the longest match list; shorter lists wrap (modulo),
///    matching JetBrains behaviour. No templates → a single empty iteration.
///    Templates that all resolve to zero matches → zero iterations (the
///    request is skipped).
pub fn expand_iterations(req: &Request, env: &Environment) -> Vec<Iteration> {
    let templates: Vec<String> = request_references(req)
        .into_iter()
        .filter(|n| n.starts_with("$."))
        .collect();
    if templates.is_empty() {
        return vec![Iteration {
            index: 0,
            bindings: Vec::new(),
            template_values: Vec::new(),
        }];
    }

    let root = env.variables_root();
    let matches: Vec<Vec<serde_json::Value>> = templates
        .iter()
        .map(|t| jsonpath::query(&root, t))
        .collect();
    let count = matches.iter().map(|m| m.len()).max().unwrap_or(0);

    let mut iterations = Vec::with_capacity(count);
    for index in 0..count {
        let mut bindings = Vec::with_capacity(templates.len());
        let mut template_values = Vec::with_capacity(templates.len());
        for (ti, template) in templates.iter().enumerate() {
            let list = &matches[ti];
            let value = if list.is_empty() {
                serde_json::Value::Null
            } else {
                list[index % list.len()].clone()
            };
            template_values.push(value.clone());
            bindings.push((template.clone(), VarValue::Json(value)));
        }
        iterations.push(Iteration {
            index,
            bindings,
            template_values,
        });
    }
    iterations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_path_basic() {
        assert_eq!(encode_path("/api/get"), "/api/get");
        assert_eq!(encode_path("/中文"), "/%E4%B8%AD%E6%96%87");
        assert_eq!(encode_path("/api%20get"), "/api%20get");
    }

    #[test]
    fn encode_path_invalid_pct() {
        // Spec is silent on `%zz`; we preserve the `%` verbatim.
        assert_eq!(encode_path("/api%zz"), "/api%zz");
    }

    #[test]
    fn format_urlencoded_multiline_pairs() {
        // example/POST.http "Send POST request with body as parameters".
        let raw = "id = 999 &\nvalue = content &\nfact = IntelliJ %+ HTTP Client %= <3";
        assert_eq!(
            format_urlencoded(raw),
            "id=999&value=content&fact=IntelliJ+%2B+HTTP+Client+%3D+%3C3"
        );
    }

    #[test]
    fn format_urlencoded_escaped_ampersand_and_percent() {
        // `%&` is a literal `&` inside a value; `%%` a literal `%`.
        let raw = "a = x%&y %% z";
        assert_eq!(format_urlencoded(raw), "a=x%26y+%25+z");
    }

    #[test]
    fn format_urlencoded_newline_separates_without_ampersand() {
        assert_eq!(format_urlencoded("a = 1\nb = 2\n"), "a=1&b=2");
    }

    #[test]
    fn prepare_urlencoded_body_is_formatted() {
        let file = crate::parser::parse_file(
            "POST https://x.test/post\nContent-Type: application/x-www-form-urlencoded\n\nid = 999 &\nvalue = content\n",
        )
        .unwrap();
        let prepared = prepare(&file.requests[0], &Environment::new()).unwrap();
        assert_eq!(
            String::from_utf8(prepared.body.unwrap()).unwrap(),
            "id=999&value=content"
        );
    }

    #[test]
    fn prepare_json_body_autoquotes_string_dynamic_variable() {
        let file = crate::parser::parse_file(
            "POST https://x.test/post\nContent-Type: application/json\n\n{\"id\": {{$random.uuid}}, \"ts\": {{$timestamp}}}\n",
        )
        .unwrap();
        let prepared = prepare(&file.requests[0], &Environment::new()).unwrap();
        let body = String::from_utf8(prepared.body.unwrap()).unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&body).unwrap_or_else(|e| panic!("invalid JSON {body:?}: {e}"));
        assert!(value["id"].is_string());
        assert!(value["ts"].is_number());
    }

    #[test]
    fn no_auto_encoding_tag_sends_target_verbatim() {
        // `@+$!` is ASCII and passes through encoding unchanged, so the tag is
        // observable with a non-ASCII character.
        let plain = crate::parser::parse_file("GET https://x.test/anything?value=€uro\n").unwrap();
        let tagged = crate::parser::parse_file(
            "# @no-auto-encoding\nGET https://x.test/anything?value=€uro\n",
        )
        .unwrap();
        let a = prepare(&plain.requests[0], &Environment::new()).unwrap();
        let b = prepare(&tagged.requests[0], &Environment::new()).unwrap();
        assert_eq!(a.url, "https://x.test/anything?value=%E2%82%ACuro");
        assert_eq!(b.url, "https://x.test/anything?value=€uro");
    }

    #[test]
    fn host_variable_may_carry_its_own_scheme() {
        // The JetBrains examples keep `scheme://host` in one variable so an
        // environment can switch between http and https.
        let file = crate::parser::parse_file("GET {{host}}/get?show_env={{show_env}}\n").unwrap();
        let mut env = Environment::new();
        env.set("host", "https://httpbin.org");
        env.set("show_env", "1");
        let prepared = prepare(&file.requests[0], &env).unwrap();
        assert_eq!(prepared.url, "https://httpbin.org/get?show_env=1");

        // Without a scheme in the value the parsed/default scheme still applies.
        let mut bare = Environment::new();
        bare.set("host", "httpbin.org");
        bare.set("show_env", "1");
        let prepared = prepare(&file.requests[0], &bare).unwrap();
        assert_eq!(prepared.url, "http://httpbin.org/get?show_env=1");

        // An explicit scheme on the request line is kept when the variable
        // holds only an authority.
        let file = crate::parser::parse_file("GET https://{{host}}/get\n").unwrap();
        let prepared = prepare(&file.requests[0], &bare).unwrap();
        assert_eq!(prepared.url, "https://httpbin.org/get");
    }

    // -- expand_iterations (plan §7.4 G10) -----------------------------------

    use crate::env::Layer;

    fn clients_env() -> Environment {
        let mut env = Environment::new();
        env.set_json(
            Layer::Environment,
            "clients",
            serde_json::json!([
                {"id": 1, "firstName": "A", "balance": 100},
                {"id": 2, "firstName": "B", "balance": 200},
                {"id": 3, "firstName": "C", "balance": 300},
            ]),
        );
        env
    }

    #[test]
    fn expand_iterations_no_jsonpath_template_is_single_empty() {
        // `{{show_env}}` is a plain variable, not a JSONPath template.
        let file = crate::parser::parse_file("GET https://x.test/get?q={{show_env}}\n").unwrap();
        let iters = expand_iterations(&file.requests[0], &clients_env());
        assert_eq!(iters.len(), 1);
        assert_eq!(iters[0].index, 0);
        assert!(iters[0].bindings.is_empty());
        assert!(iters[0].template_values.is_empty());
    }

    #[test]
    fn expand_iterations_descendant_ids_produce_three_iterations() {
        let file = crate::parser::parse_file(
            "POST https://x.test/post\nContent-Type: application/json\n\n{\"id\": {{$.clients..id}}}\n",
        )
        .unwrap();
        let iters = expand_iterations(&file.requests[0], &clients_env());
        assert_eq!(iters.len(), 3);
        for (i, it) in iters.iter().enumerate() {
            assert_eq!(it.index, i);
            assert_eq!(it.bindings.len(), 1);
            assert_eq!(it.bindings[0].0, "$.clients..id");
            assert_eq!(it.bindings[0].1, VarValue::Json(serde_json::json!(i + 1)));
            assert_eq!(it.template_values[0], serde_json::json!(i + 1));
        }
    }

    #[test]
    fn expand_iterations_multiple_templates_wrap_modulo() {
        // `$.clients..id` has 3 matches, `$.pair..v` has 2 -> 3 iterations with
        // the shorter list wrapping (modulo), matching JetBrains behaviour.
        let mut env = clients_env();
        env.set_json(
            Layer::Environment,
            "pair",
            serde_json::json!([{"v": 10}, {"v": 20}]),
        );
        let file = crate::parser::parse_file(
            "POST https://x.test/post\nContent-Type: application/json\n\n{\"id\": {{$.clients..id}}, \"p\": {{$.pair..v}}}\n",
        )
        .unwrap();
        let iters = expand_iterations(&file.requests[0], &env);
        assert_eq!(iters.len(), 3);
        // Template order follows first appearance in the request text.
        assert_eq!(iters[0].bindings[0].0, "$.clients..id");
        assert_eq!(iters[0].bindings[1].0, "$.pair..v");
        assert_eq!(iters[0].template_values[1], serde_json::json!(10));
        assert_eq!(iters[1].template_values[1], serde_json::json!(20));
        assert_eq!(iters[2].template_values[1], serde_json::json!(10)); // wraps
    }

    #[test]
    fn expand_iterations_zero_matches_yields_no_iterations() {
        let file =
            crate::parser::parse_file("GET https://x.test/get?q={{$.missing..id}}\n").unwrap();
        let iters = expand_iterations(&file.requests[0], &clients_env());
        assert!(iters.is_empty());
    }
}
