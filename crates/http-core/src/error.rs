//! Error types for http-core.

use thiserror::Error;

/// Source location: 1-based line / column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// Error category, mirrors spec sections.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// Lexer-level failure (spec ch 2).
    Lex,
    /// Parser-level failure (spec ch 3).
    Parse,
    /// Executor failure (spec ch 4).
    Execute,
    /// Response handler script failure (spec ch 4.5).
    Handler,
    /// I/O failure (file refs, response refs).
    Io,
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lex => write!(f, "lex"),
            Self::Parse => write!(f, "parse"),
            Self::Execute => write!(f, "execute"),
            Self::Handler => write!(f, "handler"),
            Self::Io => write!(f, "io"),
        }
    }
}

#[derive(Debug, Error, PartialEq, serde::Serialize, serde::Deserialize)]
#[error("{kind} error at {span}: {message}")]
pub struct CoreError {
    pub kind: ErrorKind,
    pub span: Span,
    pub message: String,
}

impl CoreError {
    pub fn new(kind: ErrorKind, span: Span, message: impl Into<String>) -> Self {
        Self {
            kind,
            span,
            message: message.into(),
        }
    }

    pub fn lex(span: Span, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Lex, span, message)
    }

    pub fn parse(span: Span, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Parse, span, message)
    }
}

/// Diagnostic emitted during parsing; does not abort the whole file.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Diagnostic {
    pub kind: ErrorKind,
    pub span: Span,
    pub message: String,
}

impl From<CoreError> for Diagnostic {
    fn from(err: CoreError) -> Self {
        Self {
            kind: err.kind,
            span: err.span,
            message: err.message,
        }
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
