//! Dispatcher: send a [`PreparedRequest`] (or a parsed [`Request`]) over the
//! wire via `reqwest` and capture the response (spec chapter 4.3 — multipart
//! dispatch — and the integration scenarios in TDD P2-5).
//!
//! The dispatcher is intentionally thin: prepare logic (env var substitution,
//! encoding, whitespace trimming, multipart assembly, file-ref resolution)
//! lives in [`crate::execute`]. Here we only translate a [`PreparedRequest`]
//! into a `reqwest::Request`, send it, and capture the response.

use crate::env::{substitute, Environment};
use crate::error::{CoreError, ErrorKind, Result, Span};
use crate::execute::{prepare_with, DiskResolver, PreparedRequest};
use crate::model::{DocTag, OutputRedirect, Request};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A captured HTTP response, ready to be displayed, persisted or compared.
#[derive(Debug, Clone)]
pub struct DispatchResponse {
    /// HTTP status code (e.g. 200, 404). 0 if no response was received
    /// (transport-level error before headers, though reqwest usually surfaces
    /// these as errors — kept here for forward-compat with chunked trailers).
    pub status: u16,
    /// Response header fields, in the order they were received. Header names
    /// preserve original case; lookups should be case-insensitive.
    pub headers: Vec<(String, String)>,
    /// Raw response body bytes. For chunked responses the full decoded body
    /// is collected here — the spec does not require streaming to the UI from
    /// the core layer.
    pub body: Vec<u8>,
    /// Total elapsed time from dispatch start to fully-read response.
    pub elapsed: Duration,
    /// Final URL after redirects (same as `url` if no redirects). Recorded
    /// for the history UI; not yet used by response handlers.
    pub url: String,
    /// HTTP version the response was received with (`HTTP/1.1`, `HTTP/2.0`, …).
    /// Rendered in the console status line.
    pub http_version: String,
}

impl DispatchResponse {
    /// Case-insensitive header lookup, returning the first match.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Per-request transport options derived from documentation tags
/// (`@no-redirect`, `@no-cookie-jar`, `@timeout`, `@connection-timeout`) and
/// the request line's HTTP version (plan §7.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendOptions {
    /// Follow 3xx redirects. JetBrains' default is *follow*; `@no-redirect`
    /// turns it off.
    pub follow_redirects: bool,
    /// Participate in the dispatcher's shared cookie jar. `@no-cookie-jar`
    /// turns it off (cookies are neither sent nor stored).
    pub cookie_jar: bool,
    /// Overall request timeout (`@timeout <millis>`).
    pub timeout: Option<Duration>,
    /// Connect-phase timeout (`@connection-timeout <millis>`).
    pub connect_timeout: Option<Duration>,
    /// Force HTTP/2 (`GET … HTTP/2`).
    pub http2: bool,
}

impl Default for SendOptions {
    fn default() -> Self {
        Self {
            follow_redirects: true,
            cookie_jar: true,
            timeout: None,
            connect_timeout: None,
            http2: false,
        }
    }
}

impl SendOptions {
    /// Derive options from a parsed request's tags and HTTP version.
    pub fn from_request(req: &Request) -> Self {
        let mut opts = Self::default();
        if req.has_tag(&DocTag::NoRedirect) {
            opts.follow_redirects = false;
        }
        if req.has_tag(&DocTag::NoCookieJar) {
            opts.cookie_jar = false;
        }
        if let Some(ms) = req.timeout_millis() {
            opts.timeout = Some(Duration::from_millis(ms));
        }
        if let Some(ms) = req.connection_timeout_millis() {
            opts.connect_timeout = Some(Duration::from_millis(ms));
        }
        if let Some(v) = &req.line.http_version {
            let v = v.to_ascii_uppercase();
            opts.http2 = v == "HTTP/2" || v == "HTTP/2.0";
        }
        opts
    }
}

/// Identity of a cached `reqwest::Client`: only client-level knobs take part
/// (redirect policy, cookie store, connect timeout). Request-level knobs
/// (timeout, HTTP version) are applied on the request builder.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ClientKey {
    follow_redirects: bool,
    cookie_jar: bool,
    connect_timeout_ms: Option<u64>,
}

/// Sends [`PreparedRequest`]s over the wire.
///
/// Wraps a pool of `reqwest::Client`s (one per distinct client-level option
/// combination) sharing a single cookie jar, so cookies set by one request are
/// visible to later ones — mirroring the JetBrains HTTP Client session.
/// [`Dispatcher::new`] uses sane defaults (follow redirects, shared cookie
/// jar, 30s timeout).
#[derive(Clone)]
pub struct Dispatcher {
    clients: Arc<Mutex<HashMap<ClientKey, reqwest::Client>>>,
    jar: Arc<reqwest::cookie::Jar>,
    /// Optional user-supplied client that bypasses the pool entirely.
    override_client: Option<reqwest::Client>,
    base_timeout: Duration,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Dispatcher {
    /// Build a dispatcher with a lazily-populated client pool (follow
    /// redirects, shared cookie jar, 30s overall timeout).
    pub fn new() -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            jar: Arc::new(reqwest::cookie::Jar::default()),
            override_client: None,
            base_timeout: Duration::from_secs(30),
        }
    }

    /// Use a pre-configured `reqwest::Client` for every request. Per-request
    /// options that are client-level (redirect policy, cookie jar, connect
    /// timeout) are then ignored; `timeout` and HTTP/2 still apply.
    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            jar: Arc::new(reqwest::cookie::Jar::default()),
            override_client: Some(client),
            base_timeout: Duration::from_secs(30),
        }
    }

    fn client_for(&self, opts: &SendOptions) -> reqwest::Client {
        if let Some(c) = &self.override_client {
            return c.clone();
        }
        let key = ClientKey {
            follow_redirects: opts.follow_redirects,
            cookie_jar: opts.cookie_jar,
            connect_timeout_ms: opts.connect_timeout.map(|d| d.as_millis() as u64),
        };
        if let Ok(mut map) = self.clients.lock() {
            if let Some(c) = map.get(&key) {
                return c.clone();
            }
            let mut builder = reqwest::Client::builder().redirect(if opts.follow_redirects {
                reqwest::redirect::Policy::limited(10)
            } else {
                reqwest::redirect::Policy::none()
            });
            if opts.cookie_jar {
                builder = builder.cookie_provider(self.jar.clone());
            }
            if let Some(ct) = opts.connect_timeout {
                builder = builder.connect_timeout(ct);
            }
            let client = builder.build().expect("reqwest client builds");
            map.insert(key, client.clone());
            client
        } else {
            // Poisoned lock: fall back to a one-off client.
            reqwest::Client::builder()
                .build()
                .expect("reqwest client builds")
        }
    }

    /// Convenience: prepare the AST [`Request`] with `env` + a [`DiskResolver`]
    /// rooted at `base_dir`, then dispatch. `base_dir` may be `None` to use
    /// the current working directory for `< file` references.
    pub async fn send(
        &self,
        req: &Request,
        env: &Environment,
        base_dir: Option<&Path>,
    ) -> Result<DispatchResponse> {
        self.send_with(req, env, base_dir, &SendOptions::from_request(req))
            .await
    }

    /// Like [`Dispatcher::send`] with explicit transport options.
    pub async fn send_with(
        &self,
        req: &Request,
        env: &Environment,
        base_dir: Option<&Path>,
        opts: &SendOptions,
    ) -> Result<DispatchResponse> {
        let resolver = match base_dir {
            Some(b) => DiskResolver::new(b),
            None => DiskResolver::default(),
        };
        let prepared = prepare_with(req, env, &resolver)?;
        self.send_prepared_with(&prepared, opts).await
    }

    /// Like [`Dispatcher::send`], but also persists the response body to the
    /// path named by the request's `<> response-ref` (spec 3.2.5 / plan §5.3
    /// step 7). If the request has no `<> ref`, this is equivalent to `send`.
    /// The file is overwritten if it already exists.
    pub async fn send_and_persist(
        &self,
        req: &Request,
        env: &Environment,
        base_dir: Option<&Path>,
    ) -> Result<DispatchResponse> {
        let res = self.send(req, env, base_dir).await?;
        if let Some(ref_path) = &req.response_ref {
            write_response_ref(&res, ref_path, base_dir)?;
        }
        Ok(res)
    }

    /// Send an already-prepared request with default options.
    pub async fn send_prepared(&self, prep: &PreparedRequest) -> Result<DispatchResponse> {
        self.send_prepared_with(prep, &SendOptions::default()).await
    }

    /// Send an already-prepared request. Use this when the caller needs to
    /// inspect the [`PreparedRequest`] (e.g. for logging) before dispatch.
    pub async fn send_prepared_with(
        &self,
        prep: &PreparedRequest,
        opts: &SendOptions,
    ) -> Result<DispatchResponse> {
        let method = method_to_reqwest(&prep.method)?;
        let client = self.client_for(opts);
        let mut builder = client
            .request(method, &prep.url)
            .header(reqwest::header::USER_AGENT, "http-client-pro/0.1")
            .timeout(opts.timeout.unwrap_or(self.base_timeout));
        if opts.http2 {
            builder = builder.version(reqwest::Version::HTTP_2);
        }

        // Headers — skip the ones reqwest owns (Host is derived from URL).
        for h in &prep.headers {
            if h.name.eq_ignore_ascii_case("Host") {
                continue;
            }
            let name = match reqwest::header::HeaderName::from_bytes(h.name.as_bytes()) {
                Ok(n) => n,
                // Skip invalid header names rather than failing the whole request;
                // emit a diagnostic? For now, just skip — the prepare step should
                // have caught anything user-visible.
                Err(_) => continue,
            };
            let value = match reqwest::header::HeaderValue::from_bytes(h.value.as_bytes()) {
                Ok(v) => v,
                Err(_) => {
                    return Err(CoreError::new(
                        ErrorKind::Execute,
                        Span::new(0, 0),
                        format!("invalid header value for `{}`: {}", h.name, h.value),
                    ));
                }
            };
            builder = builder.header(name, value);
        }

        // Body — only attach if Some(bytes) (None means no body at all, e.g.
        // a GET).
        if let Some(body) = &prep.body {
            // For multipart bodies, `execute::build_multipart_body` has
            // already assembled a single byte stream with the user-specified
            // boundary (spec 4.3 example uses `boundary=abcd`). The
            // `Content-Type: multipart/form-data; boundary=abcd` header is
            // in `prep.headers`, so we send the assembled bytes as a plain
            // body — DO NOT wrap in `reqwest::multipart::Form`, which would
            // generate its own boundary and double-wrap the body.
            builder = builder.body(body.clone());
        }

        let start = Instant::now();
        let response = builder.send().await.map_err(map_reqwest_err)?;
        let elapsed = start.elapsed();

        let status = response.status().as_u16();
        let http_version = version_label(response.version());
        let headers = response
            .headers()
            .iter()
            .map(|(n, v)| (n.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let url = response.url().to_string();
        let body = response.bytes().await.map_err(map_reqwest_err)?.to_vec();

        Ok(DispatchResponse {
            status,
            headers,
            body,
            elapsed,
            url,
            http_version,
        })
    }
}

/// Render a `reqwest::Version` the way it appears on the wire (`HTTP/1.1`).
fn version_label(v: reqwest::Version) -> String {
    let label = if v == reqwest::Version::HTTP_09 {
        "HTTP/0.9"
    } else if v == reqwest::Version::HTTP_10 {
        "HTTP/1.0"
    } else if v == reqwest::Version::HTTP_11 {
        "HTTP/1.1"
    } else if v == reqwest::Version::HTTP_2 {
        "HTTP/2.0"
    } else if v == reqwest::Version::HTTP_3 {
        "HTTP/3.0"
    } else {
        "HTTP/1.1"
    };
    label.to_string()
}

fn method_to_reqwest(m: &crate::model::Method) -> Result<reqwest::Method> {
    Ok(match m {
        crate::model::Method::Get => reqwest::Method::GET,
        crate::model::Method::Head => reqwest::Method::HEAD,
        crate::model::Method::Post => reqwest::Method::POST,
        crate::model::Method::Put => reqwest::Method::PUT,
        crate::model::Method::Delete => reqwest::Method::DELETE,
        crate::model::Method::Connect => reqwest::Method::CONNECT,
        crate::model::Method::Patch => reqwest::Method::PATCH,
        crate::model::Method::Options => reqwest::Method::OPTIONS,
        crate::model::Method::Trace => reqwest::Method::TRACE,
    })
}

fn map_reqwest_err(e: reqwest::Error) -> CoreError {
    let msg = if e.is_timeout() {
        format!("request timed out: {e}")
    } else if e.is_connect() {
        format!("connection error: {e}")
    } else if e.is_decode() {
        format!("response decode error: {e}")
    } else {
        format!("http error: {e}")
    };
    CoreError::new(ErrorKind::Execute, Span::new(0, 0), msg)
}

/// Write the response body to the file referenced by `<> response-ref`
/// (spec 3.2.5). Relative paths resolve against `base_dir` if provided,
/// otherwise against the current working directory. Overwrites an existing
/// file (per plan §5.3 step 7 "便于历史对比" — the ref is the latest snapshot).
pub fn write_response_ref(
    resp: &DispatchResponse,
    ref_path: &str,
    base_dir: Option<&Path>,
) -> Result<()> {
    let p = PathBuf::from(ref_path);
    let resolved = if p.is_absolute() {
        p
    } else if let Some(b) = base_dir {
        b.join(p)
    } else {
        p
    };
    if let Some(parent) = resolved.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CoreError::new(
                    ErrorKind::Io,
                    Span::new(0, 0),
                    format!(
                        "failed to create dir for response ref `{}`: {e}",
                        resolved.display()
                    ),
                )
            })?;
        }
    }
    std::fs::write(&resolved, &resp.body).map_err(|e| {
        CoreError::new(
            ErrorKind::Io,
            Span::new(0, 0),
            format!("failed to write response ref `{}`: {e}", resolved.display()),
        )
    })
}

/// Write the response body to the file named by a `>> path` / `>>! path`
/// output redirect (plan §7.6 G14).
///
/// - The path is env-substituted first, so `>> {{$historyFolder}}/x.json`
///   lands in `<project_root>/.http-history/x.json` (created on demand).
/// - Relative paths resolve against `base_dir`.
/// - `>>` (force = false): if the target exists, a numeric suffix is inserted
///   before the extension (`x.json` → `x-1.json` → `x-2.json` …).
/// - `>>!` (force = true): the existing file is overwritten.
///
/// Returns the path actually written.
pub fn write_output_redirect(
    resp: &DispatchResponse,
    redirect: &OutputRedirect,
    env: &Environment,
    base_dir: Option<&Path>,
) -> Result<PathBuf> {
    let substituted = substitute(&redirect.path, env);
    let p = PathBuf::from(substituted.trim());
    let mut resolved = if p.is_absolute() {
        p
    } else if let Some(b) = base_dir {
        b.join(p)
    } else {
        p
    };
    if let Some(parent) = resolved.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CoreError::new(
                    ErrorKind::Io,
                    Span::new(0, 0),
                    format!(
                        "failed to create dir for output redirect `{}`: {e}",
                        resolved.display()
                    ),
                )
            })?;
        }
    }
    if !redirect.force {
        resolved = unique_path(&resolved);
    }
    std::fs::write(&resolved, &resp.body).map_err(|e| {
        CoreError::new(
            ErrorKind::Io,
            Span::new(0, 0),
            format!(
                "failed to write output redirect `{}`: {e}",
                resolved.display()
            ),
        )
    })?;
    Ok(resolved)
}

/// First non-existing variant of `path`: `a.json` → `a-1.json` → `a-2.json` …
fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("response");
    let ext = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let suffix = ext.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    for n in 1..1000 {
        let name = if suffix.is_empty() {
            format!("{stem}-{n}")
        } else {
            format!("{stem}-{n}.{suffix}")
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    path.to_path_buf()
}

// ---------------------------------------------------------------------------
// Binary responses: detection, download file name, persistence
// ---------------------------------------------------------------------------

/// True when a `Content-Type` denotes a textual payload that a console /
/// response viewer can render inline. Anything else (spreadsheets, PDFs,
/// archives, images, `application/octet-stream`, …) is treated as a file.
pub fn is_text_content_type(content_type: &str) -> bool {
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if mime.is_empty() {
        return false;
    }
    if mime.starts_with("text/") || mime.ends_with("+json") || mime.ends_with("+xml") {
        return true;
    }
    matches!(
        mime.as_str(),
        "application/json"
            | "application/xml"
            | "application/javascript"
            | "application/ecmascript"
            | "application/x-www-form-urlencoded"
            | "application/x-ndjson"
            | "application/csv"
            | "application/sql"
            | "application/graphql"
            | "application/manifest+json"
            | "application/x-sh"
    )
}

/// Sniff a body when no usable `Content-Type` is present: invalid UTF-8, a NUL
/// byte or a high share of control characters all mean "not text".
pub fn body_looks_binary(body: &[u8]) -> bool {
    if body.is_empty() {
        return false;
    }
    // Sample a char-boundary-safe prefix so a multibyte char split at the cut
    // is not mistaken for invalid UTF-8.
    let mut end = body.len().min(8192);
    // Back off while `end` lands on a UTF-8 continuation byte (0b10xxxxxx) so a
    // multibyte char split at the cut is not mistaken for invalid UTF-8.
    while end > 0 && end < body.len() && (body[end] & 0b1100_0000) == 0b1000_0000 {
        end -= 1;
    }
    let sample = &body[..end];
    if sample.contains(&0) || std::str::from_utf8(sample).is_err() {
        return true;
    }
    let control = sample
        .iter()
        .filter(|b| **b < 0x20 && !matches!(**b, b'\t' | b'\n' | b'\r'))
        .count();
    control * 100 > sample.len()
}

/// True when the response payload should be saved as a file instead of being
/// rendered as text — the JetBrains HTTP Client behaviour for downloads
/// (`Content-Disposition: attachment`, a non-textual `Content-Type`, or a body
/// that does not look like text).
pub fn is_binary_response(resp: &DispatchResponse) -> bool {
    if let Some(cd) = resp.header("content-disposition") {
        if cd.to_ascii_lowercase().contains("attachment") {
            return true;
        }
    }
    match resp.header("content-type") {
        Some(ct) if !ct.trim().is_empty() => !is_text_content_type(ct),
        _ => body_looks_binary(&resp.body),
    }
}

/// Best-effort download file name for a response:
/// 1. `Content-Disposition` — RFC 5987 `filename*=utf-8''%E2%80%A6` wins over a
///    plain `filename="…"`.
/// 2. The last path segment of the final URL.
/// 3. `response.<ext>` with `<ext>` guessed from the MIME type.
///
/// The name is sanitised: server-supplied values must not be able to escape
/// the target directory.
pub fn suggested_file_name(resp: &DispatchResponse) -> String {
    let candidates = resp
        .header("content-disposition")
        .and_then(filename_from_disposition)
        .into_iter()
        .chain(file_name_from_url(&resp.url))
        .map(|name| sanitize_file_name(&name))
        .find(|name| !name.is_empty());
    candidates.unwrap_or_else(|| format!("response.{}", extension_for_mime(resp)))
}

/// Save a binary response body under `dir`, named after
/// [`suggested_file_name`] with a `-1`, `-2`, … suffix when that name is
/// already taken (each run keeps its own copy, like JetBrains). `dir` is
/// created on demand. Returns the path written.
pub fn save_binary_response(resp: &DispatchResponse, dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).map_err(|e| {
        CoreError::new(
            ErrorKind::Io,
            Span::new(0, 0),
            format!(
                "failed to create binary response dir `{}`: {e}",
                dir.display()
            ),
        )
    })?;
    let path = unique_path(&dir.join(suggested_file_name(resp)));
    std::fs::write(&path, &resp.body).map_err(|e| {
        CoreError::new(
            ErrorKind::Io,
            Span::new(0, 0),
            format!(
                "failed to save binary response `{}`: {e}",
                path.display()
            ),
        )
    })?;
    Ok(path)
}

/// Extract the file name from a `Content-Disposition` header value.
fn filename_from_disposition(value: &str) -> Option<String> {
    let mut plain = None;
    for part in value.split(';') {
        let Some((key, raw)) = part.trim().split_once('=') else {
            continue;
        };
        let raw = raw.trim().trim_matches('"');
        if raw.is_empty() {
            continue;
        }
        match key.trim().to_ascii_lowercase().as_str() {
            // RFC 5987 extended parameter: `charset'language'percent-encoded`.
            "filename*" => {
                let encoded = raw.split_once("''").map_or(raw, |(_, rest)| rest);
                return Some(
                    percent_encoding::percent_decode_str(encoded)
                        .decode_utf8_lossy()
                        .into_owned(),
                );
            }
            "filename" if plain.is_none() => plain = Some(raw.to_string()),
            _ => {}
        }
    }
    plain
}

/// Last path segment of a URL, percent-decoded (`…/a/report.xlsx?x=1`).
fn file_name_from_url(url: &str) -> Option<String> {
    let bare = url.split(['?', '#']).next().unwrap_or(url);
    let last = bare.rsplit('/').next()?.trim();
    if last.is_empty() {
        return None;
    }
    Some(
        percent_encoding::percent_decode_str(last)
            .decode_utf8_lossy()
            .into_owned(),
    )
}

/// Reduce a server-supplied name to a single safe path component. Returns an
/// empty string when nothing usable is left.
fn sanitize_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?') {
                '_'
            } else {
                c
            }
        })
        .collect();
    // Trailing dots/spaces are invalid on Windows; a bare "." escapes nothing
    // but is not a file name either.
    let cleaned = cleaned.trim_matches(|c| c == '.' || c == ' ').to_string();
    if cleaned.is_empty() || cleaned.starts_with('.') {
        String::new()
    } else {
        cleaned
    }
}

/// File extension for a MIME type, used when neither `Content-Disposition` nor
/// the URL yields a name.
fn extension_for_mime(resp: &DispatchResponse) -> &'static str {
    let mime = resp
        .header("content-type")
        .map(|ct| ct.split(';').next().unwrap_or("").trim().to_ascii_lowercase())
        .unwrap_or_default();
    match mime.as_str() {
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => "pptx",
        "application/vnd.ms-excel" => "xls",
        "application/pdf" => "pdf",
        "application/zip" => "zip",
        "application/gzip" | "application/x-gzip" => "gz",
        "application/json" => "json",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "text/csv" => "csv",
        "text/plain" => "txt",
        "text/html" => "html",
        _ => "bin",
    }
}

/// Build the JSON payload sent over the wire (Tauri IPC / http-web REST /
/// SSE). Text responses inline their body; binary responses (a download) are
/// written under `save_dir` — defaulting to `.http-history` in the process CWD
/// — and described by `binary` / `file_name` / `saved_path` instead of dumping
/// raw bytes into the console (the JetBrains HTTP Client behaviour).
///
/// Shape: `{ status, headers, body, elapsed_ms, url, http_version,
/// content_length, binary, file_name, saved_path }`.
pub fn response_to_wire(res: &DispatchResponse, save_dir: Option<&Path>) -> Value {
    let headers = res
        .headers
        .iter()
        .map(|(n, v)| (n.clone(), Value::from(v.clone())))
        .collect::<serde_json::Map<String, Value>>();

    let mut obj = serde_json::Map::new();
    obj.insert("status".into(), json!(res.status));
    obj.insert("headers".into(), Value::Object(headers));
    obj.insert("elapsed_ms".into(), json!(res.elapsed.as_millis() as u64));
    obj.insert("url".into(), json!(res.url));
    obj.insert("http_version".into(), json!(res.http_version));
    obj.insert("content_length".into(), json!(res.body.len() as u64));

    if is_binary_response(res) {
        let dir = save_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".http-history"));
        obj.insert("binary".into(), json!(true));
        match save_binary_response(res, &dir) {
            Ok(path) => {
                // Display the de-duplicated name actually written (`a-1.xlsx`).
                let name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| suggested_file_name(res));
                obj.insert("file_name".into(), json!(name));
                obj.insert("saved_path".into(), json!(path.to_string_lossy()));
            }
            Err(e) => {
                obj.insert("file_name".into(), json!(suggested_file_name(res)));
                obj.insert("saved_path".into(), Value::Null);
                obj.insert("save_error".into(), json!(e.to_string()));
            }
        }
        // Never inline raw download bytes.
        obj.insert("body".into(), json!(""));
    } else {
        obj.insert("binary".into(), json!(false));
        obj.insert("file_name".into(), Value::Null);
        obj.insert("saved_path".into(), Value::Null);
        obj.insert("body".into(), json!(String::from_utf8_lossy(&res.body)));
    }

    Value::Object(obj)
}
