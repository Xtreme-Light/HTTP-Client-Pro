//! HTTP Request in Editor format core.
//!
//! See `spec.md` for the format specification and `plan.md` for architecture.
//!
//! Modules (landed incrementally):
//! - `lexer`  — spec chapter 2 (lexical structure)
//! - `parser` — spec chapter 3 (grammar structure)
//! - `execute` — spec chapter 4 (execution)
//! - `handler` — spec chapter 4.5 (response handler script)

pub mod dispatch;
pub mod dynamic;
pub mod env;
pub mod error;
pub mod execute;
pub mod jsonpath;
pub mod lexer;
pub mod model;
pub mod parser;

#[cfg(feature = "js-handler")]
pub mod handler;
#[cfg(feature = "js-handler")]
pub mod runner;
