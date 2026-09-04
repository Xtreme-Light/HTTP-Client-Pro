//! Line-based lexer for HTTP Request in Editor format (spec ch 2).
//!
//! Produces a stream of [`Line`] values. Each physical line is classified into
//! one of: `Separator`, `Comment`, `Empty`, `Indented`, `Content`.
//!
//! Why line-based rather than a fine-grained token stream?
//!
//! - spec 2.2 `new-line-with-indent` maps directly to an `Indented` line;
//! - comments (spec 2.4) only start "from the beginning of the line with or
//!   without indent", so they are a line-level construct;
//! - the parser consumes whole lines (request-line, header field, body line)
//!   and reconstructs multiline values itself.
//!
//! New-line forms (LF / CR / CRLF) are normalized (spec 2.2). Trailing
//! whitespace on comment / separator lines is trimmed.

use crate::error::Span;

/// A classified physical line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// `###` (+ optional comment), spec 2.5. Comment text is trimmed.
    Separator { comment: Option<String>, span: Span },
    /// `#` or `//` line comment, spec 2.4. Payload is the comment text
    /// (after the marker) trimmed of trailing whitespace.
    Comment { text: String, span: Span },
    /// A blank line (empty or whitespace-only), spec 3.2 `headers new-line`.
    Empty { span: Span },
    /// new-line-with-indent continuation, spec 2.2. `indent` is the leading
    /// whitespace run; `text` is the remainder of the line (no trailing newline).
    Indented {
        indent: String,
        text: String,
        span: Span,
    },
    /// A regular content line — starts with a non-whitespace, non-comment
    /// character. `text` does not include the trailing newline.
    Content { text: String, span: Span },
}

impl Line {
    pub fn span(&self) -> Span {
        match self {
            Self::Separator { span, .. }
            | Self::Comment { span, .. }
            | Self::Empty { span }
            | Self::Indented { span, .. }
            | Self::Content { span, .. } => *span,
        }
    }

    /// Convenience constructor for tests.
    pub fn content(text: impl Into<String>) -> Self {
        Self::Content {
            text: text.into(),
            span: Span::new(0, 0),
        }
    }

    /// Convenience constructor for tests.
    pub fn indented(indent: impl Into<String>, text: impl Into<String>) -> Self {
        Self::Indented {
            indent: indent.into(),
            text: text.into(),
            span: Span::new(0, 0),
        }
    }

    /// Convenience constructor for tests.
    pub fn comment(text: impl Into<String>) -> Self {
        Self::Comment {
            text: text.into(),
            span: Span::new(0, 0),
        }
    }

    /// Convenience constructor for tests.
    pub fn separator(comment: Option<String>) -> Self {
        Self::Separator {
            comment,
            span: Span::new(0, 0),
        }
    }

    /// Convenience constructor for tests.
    pub fn empty() -> Self {
        Self::Empty {
            span: Span::new(0, 0),
        }
    }
}

pub struct Lexer<'a> {
    /// Original source, retained for richer diagnostics (e.g. byte offsets).
    #[allow(dead_code)]
    src: &'a str,
    chars: Vec<char>,
    pos: usize,
    line: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            chars: src.chars().collect(),
            pos: 0,
            line: 0,
        }
    }

    fn current_span(&self) -> Span {
        Span::new(self.line + 1, 1)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
        }
        Some(c)
    }

    /// Detect a new-line sequence (LF / CR / CRLF) at the current position.
    /// Returns the length in chars (1 or 2) if matched, else 0.
    fn match_new_line(&self) -> usize {
        match (self.peek(), self.peek_at(1)) {
            (Some('\r'), Some('\n')) => 2,
            (Some('\r'), _) | (Some('\n'), _) => 1,
            _ => 0,
        }
    }

    fn is_whitespace(ch: char) -> bool {
        // spec 2.3: SP / HT / FF
        matches!(ch, ' ' | '\t' | '\u{000C}')
    }

    /// Read until end of line (consuming the newline). Returns the line text
    /// excluding the trailing newline. Does not consume the newline of an
    /// already-EOF line.
    fn read_line_text(&mut self) -> String {
        let mut out = String::new();
        while let Some(ch) = self.peek() {
            if ch == '\n' || ch == '\r' {
                break;
            }
            out.push(ch);
            self.advance();
        }
        out
    }

    fn consume_newline(&mut self) {
        let n = self.match_new_line();
        for _ in 0..n {
            self.advance();
        }
    }

    /// Trim trailing whitespace (spec: trailing whitespace on comments is
    /// insignificant; for content/indented lines, parser does its own trim).
    fn trim_trailing_ws(s: &str) -> &str {
        s.trim_end_matches([' ', '\t', '\u{000C}'])
    }

    fn next_line(&mut self) -> Option<Line> {
        if self.pos >= self.chars.len() {
            return None;
        }
        let span = self.current_span();

        // Classify line by its first char(s).
        // Order matters: `###` must beat `#` comment.
        let is_separator = self.peek() == Some('#')
            && self.peek_at(1) == Some('#')
            && self.peek_at(2) == Some('#');
        let is_hash_comment = !is_separator && self.peek() == Some('#');
        let is_slash_comment = self.peek() == Some('/') && self.peek_at(1) == Some('/');

        if is_separator {
            // consume `###`
            self.advance();
            self.advance();
            self.advance();
            let rest = self.read_line_text();
            self.consume_newline();
            let trimmed = Self::trim_trailing_ws(&rest).trim_start();
            let comment = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            };
            return Some(Line::Separator { comment, span });
        }

        if is_hash_comment {
            self.advance(); // consume `#`
            let rest = self.read_line_text();
            self.consume_newline();
            let trimmed = Self::trim_trailing_ws(&rest).trim_start();
            return Some(Line::Comment {
                text: trimmed.to_string(),
                span,
            });
        }

        if is_slash_comment {
            self.advance();
            self.advance(); // consume `//`
            let rest = self.read_line_text();
            self.consume_newline();
            let trimmed = Self::trim_trailing_ws(&rest).trim_start();
            return Some(Line::Comment {
                text: trimmed.to_string(),
                span,
            });
        }

        // Empty or whitespace-only line → Empty.
        let leading_is_ws = self.peek().map(Self::is_whitespace).unwrap_or(false);
        if leading_is_ws {
            let line_text = self.read_line_text();
            self.consume_newline();
            if line_text.trim().is_empty() {
                return Some(Line::Empty { span });
            }
            // Whitespace + content → Indented.
            let mut indent = String::new();
            let mut text = String::new();
            let mut chars = line_text.chars().peekable();
            while let Some(&ch) = chars.peek() {
                if Self::is_whitespace(ch) {
                    indent.push(ch);
                    chars.next();
                } else {
                    break;
                }
            }
            for ch in chars {
                text.push(ch);
            }
            return Some(Line::Indented { indent, text, span });
        }

        // Regular content line.
        let text = self.read_line_text();
        self.consume_newline();
        // Drop an empty trailing case (shouldn't happen because peek is Some).
        if text.is_empty() {
            return Some(Line::Empty { span });
        }
        Some(Line::Content { text, span })
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Line;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_line()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_preserved_for_diagnostics() {
        let lx = Lexer::new("a\nb");
        assert_eq!(lx.src, "a\nb");
    }
}
