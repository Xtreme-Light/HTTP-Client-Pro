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
        })
    }
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
