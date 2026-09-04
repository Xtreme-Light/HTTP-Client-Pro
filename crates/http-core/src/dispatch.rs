//! Dispatcher: send a [`PreparedRequest`] (or a parsed [`Request`]) over the
//! wire via `reqwest` and capture the response (spec chapter 4.3 — multipart
//! dispatch — and the integration scenarios in TDD P2-5).
//!
//! The dispatcher is intentionally thin: prepare logic (env var substitution,
//! encoding, whitespace trimming, multipart assembly, file-ref resolution)
//! lives in [`crate::execute`]. Here we only translate a [`PreparedRequest`]
//! into a `reqwest::Request`, send it, and capture the response.

use crate::env::Environment;
use crate::error::{CoreError, ErrorKind, Result, Span};
use crate::execute::{prepare_with, DiskResolver, PreparedRequest};
use crate::model::Request;
use std::path::{Path, PathBuf};
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

/// Sends [`PreparedRequest`]s over the wire.
///
/// Wraps a `reqwest::Client` so the caller can configure TLS, timeouts,
/// proxies, redirect policy, etc. [`Dispatcher::new`] uses a sane default
/// client (no redirects followed, 30s timeout).
#[derive(Clone)]
pub struct Dispatcher {
    client: reqwest::Client,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Dispatcher {
    /// Build a dispatcher with a default `reqwest::Client` (no redirects,
    /// 30s overall timeout). For custom configuration, construct the
    /// `reqwest::Client` yourself and use [`Dispatcher::with_client`].
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .build()
            .expect("default reqwest client builds");
        Self { client }
    }

    /// Use a pre-configured `reqwest::Client`.
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
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
        let resolver = match base_dir {
            Some(b) => DiskResolver::new(b),
            None => DiskResolver::default(),
        };
        let prepared = prepare_with(req, env, &resolver)?;
        self.send_prepared(&prepared).await
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

    /// Send an already-prepared request. Use this when the caller needs to
    /// inspect the [`PreparedRequest`] (e.g. for logging) before dispatch.
    pub async fn send_prepared(&self, prep: &PreparedRequest) -> Result<DispatchResponse> {
        let method = method_to_reqwest(&prep.method)?;
        let mut builder = self
            .client
            .request(method, &prep.url)
            .header(reqwest::header::USER_AGENT, "http-client-pro/0.1");

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
