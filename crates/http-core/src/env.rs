//! Environment variables (spec 3.2.6 / 4.4).
//!
//! An `Environment` is a flat map of `identifier -> value`. Substitution
//! scans text for `{{ ... }}` references (spec 3.2.6 `env-variable`) and
//! replaces them with the environment value, preserving the literal
//! `{{name}}` if undefined.

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct Environment {
    vars: HashMap<String, String>,
}

impl Environment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) {
        self.vars.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }
}

/// Look up `{{name}}` references in `text`, returning the resolved value or
/// the original `{{name}}` literal if `name` is not in `env`.
///
/// Recognises optional whitespace inside the braces (spec 3.2.6).
/// Substituted values are NOT re-encoded — they are inserted verbatim.
pub fn substitute(text: &str, env: &Environment) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.char_indices().collect::<Vec<_>>();
    let mut i = 0;
    while i < bytes.len() {
        let (_, ch) = bytes[i];
        if ch == '{' && i + 1 < bytes.len() && bytes[i + 1].1 == '{' {
            // Try to match a complete `{{ [ws] identifier [ws] }}`.
            if let Some((end, name)) = scan_env_var(&bytes, i) {
                let value = env.get(&name).map(str::to_string).unwrap_or_else(|| {
                    // Preserve the original span (incl. whitespace) verbatim.
                    text[bytes[i].0..bytes[end].0 + bytes[end].1.len_utf8()].to_string()
                });
                out.push_str(&value);
                i = end + 1;
                continue;
            }
        }
        out.push(ch);
        i += 1;
    }
    out
}

/// If `bytes[start]` begins a `{{ [ws] identifier [ws] }}` sequence, return
/// the index of the closing `}` and the trimmed identifier.
fn scan_env_var(bytes: &[(usize, char)], start: usize) -> Option<(usize, String)> {
    // bytes[start] == '{', bytes[start+1] == '{'.
    let mut i = start + 2;
    // skip optional whitespace
    while i < bytes.len() && bytes[i].1.is_whitespace() {
        i += 1;
    }
    let id_start = i;
    // identifier = (ident-char)+ where ident-char = alpha | digit | '-' | '_'
    while i < bytes.len() {
        let c = bytes[i].1;
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            i += 1;
        } else {
            break;
        }
    }
    let id_end = i;
    if id_end == id_start {
        return None; // empty identifier
    }
    // skip optional whitespace
    while i < bytes.len() && bytes[i].1.is_whitespace() {
        i += 1;
    }
    // expect `}}`
    if i + 1 < bytes.len() && bytes[i].1 == '}' && bytes[i + 1].1 == '}' {
        let name: String = bytes[id_start..id_end].iter().map(|(_, c)| *c).collect();
        Some((i + 1, name))
    } else {
        None
    }
}

/// Collect the set of identifiers referenced via `{{ ... }}` in `text`.
/// Useful for the editor UI to highlight undefined variables.
pub fn collect_references(text: &str) -> Vec<String> {
    let bytes = text.char_indices().collect::<Vec<_>>();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let (_, ch) = bytes[i];
        if ch == '{' && i + 1 < bytes.len() && bytes[i + 1].1 == '{' {
            if let Some((end, name)) = scan_env_var(&bytes, i) {
                out.push(name);
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_simple() {
        let mut env = Environment::new();
        env.set("host", "example.com");
        assert_eq!(
            substitute("http://{{host}}/api", &env),
            "http://example.com/api"
        );
    }

    #[test]
    fn substitute_with_internal_whitespace() {
        let mut env = Environment::new();
        env.set("host", "example.com");
        assert_eq!(
            substitute("http://{{ host }}/api", &env),
            "http://example.com/api"
        );
    }

    #[test]
    fn substitute_undefined_preserved() {
        let env = Environment::new();
        assert_eq!(
            substitute("http://{{unknown}}/api", &env),
            "http://{{unknown}}/api"
        );
    }

    #[test]
    fn substitute_undefined_with_whitespace_preserved_verbatim() {
        let env = Environment::new();
        assert_eq!(substitute("{{ x }}", &env), "{{ x }}");
    }

    #[test]
    fn substitute_multiple_in_one_string() {
        let mut env = Environment::new();
        env.set("a", "X");
        env.set("b", "Y");
        assert_eq!(substitute("{{a}}+{{b}}", &env), "X+Y");
    }

    #[test]
    fn collect_references_basic() {
        let refs = collect_references("http://{{host}}/api?q={{id}}");
        assert_eq!(refs, vec!["host".to_string(), "id".to_string()]);
    }

    #[test]
    fn collect_references_dedup_is_caller_responsibility() {
        // The function returns references in order of appearance; dedup is up
        // to the caller (the UI may want counts).
        let refs = collect_references("{{a}}{{a}}{{b}}");
        assert_eq!(
            refs,
            vec!["a".to_string(), "a".to_string(), "b".to_string()]
        );
    }
}
