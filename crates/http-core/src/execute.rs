//! Execution: turn an AST [`Request`] into a [`PreparedRequest`] ready for
//! dispatch (spec chapter 4.1~4.3).
//!
//! Pipeline: substitute env vars (4.4) → encode path/query (4.1.2) → trim
//! multiline path segments (4.2.1) → assemble URL → encode body (4.1.3) →
//! trim in-place body (4.2.2). Dispatch against a real HTTP client lives
//! in [`crate::dispatch`].

use crate::env::{substitute, Environment};
use crate::error::{CoreError, ErrorKind, Result, Span};
use crate::model::{
    HeaderField, MessageBody, MessagePartBody, Method, MultipartField, Request, RequestTarget,
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
    let url = build_url(&req.line.target, env, &headers)?;

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

fn build_url(target: &RequestTarget, env: &Environment, headers: &[HeaderField]) -> Result<String> {
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
            let path_enc = encode_path(&substitute(path, env));
            let mut url = format!("http://{host}{path_enc}");
            if let Some(q) = query {
                url.push('?');
                url.push_str(&encode_query(&substitute(q, env)));
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
            let mut url = format!("{scheme}://{auth}");
            if let Some(p) = path {
                url.push_str(&encode_path(&substitute(p, env)));
            }
            if let Some(q) = query {
                url.push('?');
                url.push_str(&encode_query(&substitute(q, env)));
            }
            if let Some(f) = fragment {
                url.push('#');
                url.push_str(&substitute(f, env));
            }
            Ok(url)
        }
    }
}

fn build_body(
    body: &Option<MessageBody>,
    env: &Environment,
    _headers: &[HeaderField],
    resolver: &dyn FileResolver,
) -> Result<(Option<Vec<u8>>, Option<String>)> {
    match body {
        None => Ok((None, None)),
        Some(MessageBody::Inline { content }) => {
            // spec 4.2.2: in-place body whitespace around it is trimmed
            // (the parser already trims; we re-trim post-substitution to be
            // safe in case substitution introduced leading/trailing ws).
            let substituted = substitute(content, env);
            let trimmed = substituted.trim();
            if trimmed.is_empty() {
                Ok((Some(Vec::new()), None))
            } else {
                // spec 4.1.3: default encoding is UTF-8 — String is already
                // UTF-8 in Rust, so bytes are correct.
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
}
