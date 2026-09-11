//! AST / IR types for HTTP Request in Editor format (spec ch 3).
//!
//! Populated incrementally in Phase 1.

use crate::error::Diagnostic;
use serde::{Deserialize, Serialize};

/// A parsed `.http` file (spec 3.1).
///
/// Parsing is lenient: a malformed request is recorded as a [`Diagnostic`]
/// and skipped, so the rest of the file still parses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestsFile {
    pub requests: Vec<Request>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub line: RequestLine,
    #[serde(default)]
    pub headers: Vec<HeaderField>,
    #[serde(default)]
    pub body: Option<MessageBody>,
    #[serde(default)]
    pub response_handler: Option<ResponseHandler>,
    #[serde(default)]
    pub response_ref: Option<String>,
    /// Optional name from the preceding `### name` separator comment.
    /// `None` for unnamed requests; populated by the parser from the
    /// separator immediately above this request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Pre-request script (`< {% ... %}`), executed before dispatch.
    /// Shares the [`ResponseHandler`] shape so `< script.js` also works.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_request_script: Option<ResponseHandler>,
    /// Response output redirection (`>> file` / `>>! file`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_redirect: Option<OutputRedirect>,
    /// Documentation tags harvested from the comments above the request
    /// (`# @no-redirect`, `# @timeout 5000`, …).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<DocTag>,
}

impl Request {
    pub fn has_tag(&self, tag: &DocTag) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// First `@timeout` value in milliseconds, if declared.
    pub fn timeout_millis(&self) -> Option<u64> {
        self.tags.iter().find_map(|t| match t {
            DocTag::Timeout { millis } => Some(*millis),
            _ => None,
        })
    }

    /// First `@connection-timeout` value in milliseconds, if declared.
    pub fn connection_timeout_millis(&self) -> Option<u64> {
        self.tags.iter().find_map(|t| match t {
            DocTag::ConnectionTimeout { millis } => Some(*millis),
            _ => None,
        })
    }
}

/// `>> path` (append-suffix on collision) / `>>! path` (force overwrite).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputRedirect {
    pub path: String,
    pub force: bool,
}

/// Request-scoping documentation tags (JetBrains HTTP Client extension).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tag", rename_all = "kebab-case")]
pub enum DocTag {
    /// `@no-redirect` — do not follow 3xx.
    NoRedirect,
    /// `@no-cookie-jar` — neither send nor store cookies.
    NoCookieJar,
    /// `@no-auto-encoding` — send path/query verbatim.
    NoAutoEncoding,
    /// `@no-log` — suppress request/response logging.
    NoLog,
    /// `@timeout <millis>` — overall request timeout.
    Timeout { millis: u64 },
    /// `@connection-timeout <millis>` — connect-phase timeout.
    ConnectionTimeout { millis: u64 },
    /// `@name <text>` — explicit request name.
    Name { value: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestLine {
    pub method: Method,
    pub target: RequestTarget,
    pub http_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Connect,
    Patch,
    Options,
    Trace,
}

impl Method {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Connect => "CONNECT",
            Self::Patch => "PATCH",
            Self::Options => "OPTIONS",
            Self::Trace => "TRACE",
        }
    }
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RequestTarget {
    Origin {
        path: String,
        query: Option<String>,
        fragment: Option<String>,
    },
    Absolute {
        scheme: Option<String>,
        authority: String,
        path: Option<String>,
        query: Option<String>,
        fragment: Option<String>,
    },
    Asterisk,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeaderField {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessageBody {
    /// Inline message lines (spec 3.2.3 `messages`).
    Inline { content: String },
    /// `< file-path` (spec 3.2.3 `input-file-ref`).
    FileRef { path: String },
    /// `multipart/form-data` (spec 3.2.3.1).
    Multipart {
        boundary: String,
        parts: Vec<MultipartField>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultipartField {
    #[serde(default)]
    pub headers: Vec<HeaderField>,
    pub body: MessagePartBody,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessagePartBody {
    Inline { content: String },
    FileRef { path: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResponseHandler {
    Inline { script: String },
    FileRef { path: String },
}
