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
