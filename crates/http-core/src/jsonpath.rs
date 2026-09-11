//! Minimal JSONPath evaluator (plan §7.4 G9).
//!
//! Supports the subset used by the JetBrains examples and handlers:
//! - `$`            — root
//! - `.field`       — child member by name
//! - `['field']`    — child member by quoted name
//! - `[n]`          — array index (negative counts from the end)
//! - `[*]`          — wildcard over array elements / object members
//! - `..field`      — recursive descent, collecting every member named `field`
//! - `..*`          — recursive descent over all descendants
//!
//! [`query`] returns every match in document order; single-value callers take
//! the first. A deliberately small hand-rolled evaluator avoids pulling in a
//! third-party crate whose semantics might diverge from JetBrains'.

use serde_json::Value;

/// Evaluate a JSONPath `expr` against `root`, returning all matches in
/// document order. An empty result means "no match" (JetBrains treats a
/// multi-value template with zero matches as zero iterations).
pub fn query(root: &Value, expr: &str) -> Vec<Value> {
    let expr = expr.trim();
    let tokens = match tokenize(expr) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let mut current = vec![root.clone()];
    for tok in tokens {
        let mut next = Vec::new();
        for node in &current {
            apply(node, &tok, &mut next);
        }
        current = next;
    }
    current
}

/// Evaluate and collapse to a single value (first match), if any.
pub fn query_one(root: &Value, expr: &str) -> Option<Value> {
    query(root, expr).into_iter().next()
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    /// `.field` — child member.
    Child(String),
    /// `[n]` — array index.
    Index(i64),
    /// `[*]` — wildcard.
    Wildcard,
    /// `..field` — recursive descent collecting members named `field`.
    Descend(String),
    /// `..*` — recursive descent over all descendants.
    DescendAll,
}

fn tokenize(expr: &str) -> Option<Vec<Token>> {
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 0;
    // Must start with `$`.
    if chars.first() != Some(&'$') {
        return None;
    }
    i += 1;
    let mut out = Vec::new();
    while i < chars.len() {
        match chars[i] {
            '.' => {
                i += 1;
                if i < chars.len() && chars[i] == '.' {
                    // `..` recursive descent
                    i += 1;
                    if i < chars.len() && chars[i] == '*' {
                        out.push(Token::DescendAll);
                        i += 1;
                    } else {
                        let (name, ni) = read_name(&chars, i);
                        if name.is_empty() {
                            return None;
                        }
                        out.push(Token::Descend(name));
                        i = ni;
                    }
                } else if i < chars.len() && chars[i] == '*' {
                    out.push(Token::Wildcard);
                    i += 1;
                } else {
                    let (name, ni) = read_name(&chars, i);
                    if name.is_empty() {
                        return None;
                    }
                    out.push(Token::Child(name));
                    i = ni;
                }
            }
            '[' => {
                i += 1;
                // find closing ']'
                let start = i;
                while i < chars.len() && chars[i] != ']' {
                    i += 1;
                }
                if i >= chars.len() {
                    return None; // unterminated
                }
                let inner: String = chars[start..i].iter().collect();
                i += 1; // skip ']'
                let inner = inner.trim();
                if inner == "*" {
                    out.push(Token::Wildcard);
                } else if (inner.starts_with('\'') && inner.ends_with('\'') && inner.len() >= 2)
                    || (inner.starts_with('"') && inner.ends_with('"') && inner.len() >= 2)
                {
                    out.push(Token::Child(inner[1..inner.len() - 1].to_string()));
                } else if let Ok(n) = inner.parse::<i64>() {
                    out.push(Token::Index(n));
                } else {
                    return None;
                }
            }
            _ => return None, // unexpected char
        }
    }
    Some(out)
}

/// Read an identifier (letters, digits, `_`, `-`) starting at `i`.
fn read_name(chars: &[char], mut i: usize) -> (String, usize) {
    let start = i;
    while i < chars.len() {
        let c = chars[i];
        if c.is_alphanumeric() || c == '_' || c == '-' {
            i += 1;
        } else {
            break;
        }
    }
    (chars[start..i].iter().collect(), i)
}

fn apply(node: &Value, tok: &Token, out: &mut Vec<Value>) {
    match tok {
        Token::Child(name) => {
            if let Value::Object(map) = node {
                if let Some(v) = map.get(name) {
                    out.push(v.clone());
                }
            }
        }
        Token::Index(n) => {
            if let Value::Array(arr) = node {
                let idx = if *n < 0 {
                    arr.len() as i64 + n
                } else {
                    *n
                };
                if idx >= 0 && (idx as usize) < arr.len() {
                    out.push(arr[idx as usize].clone());
                }
            }
        }
        Token::Wildcard => match node {
            Value::Array(arr) => out.extend(arr.iter().cloned()),
            Value::Object(map) => out.extend(map.values().cloned()),
            _ => {}
        },
        Token::Descend(name) => descend(node, Some(name), out),
        Token::DescendAll => descend(node, None, out),
    }
}

/// Depth-first collection. When `name` is `Some`, collect every object member
/// with that key (the member's value). When `None`, collect every descendant
/// node.
fn descend(node: &Value, name: Option<&str>, out: &mut Vec<Value>) {
    match node {
        Value::Array(arr) => {
            for item in arr {
                descend(item, name, out);
            }
        }
        Value::Object(map) => {
            for (k, v) in map {
                match name {
                    Some(n) => {
                        if k == n {
                            out.push(v.clone());
                        }
                    }
                    None => out.push(v.clone()),
                }
                descend(v, name, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn child_path() {
        let root = json!({"json": {"balance": 100}});
        assert_eq!(query_one(&root, "$.json.balance"), Some(json!(100)));
    }

    #[test]
    fn recursive_descent_collects_all() {
        let root = json!({"clients": [
            {"id": 1, "firstName": "George"},
            {"id": 2, "firstName": "John"},
            {"id": 3, "firstName": "Eduardo"}
        ]});
        let ids = query(&root, "$.clients..id");
        assert_eq!(ids, vec![json!(1), json!(2), json!(3)]);
        let names = query(&root, "$.clients..firstName");
        assert_eq!(
            names,
            vec![json!("George"), json!("John"), json!("Eduardo")]
        );
    }

    #[test]
    fn index_and_negative() {
        let root = json!({"a": [10, 20, 30]});
        assert_eq!(query_one(&root, "$.a[0]"), Some(json!(10)));
        assert_eq!(query_one(&root, "$.a[-1]"), Some(json!(30)));
    }

    #[test]
    fn wildcard() {
        let root = json!({"a": [1, 2, 3]});
        assert_eq!(query(&root, "$.a[*]"), vec![json!(1), json!(2), json!(3)]);
    }

    #[test]
    fn quoted_child() {
        let root = json!({"a": {"b c": 1}});
        assert_eq!(query_one(&root, "$.a['b c']"), Some(json!(1)));
    }

    #[test]
    fn no_match_empty() {
        let root = json!({"a": 1});
        assert!(query(&root, "$.b").is_empty());
        assert!(query(&root, "not-a-path").is_empty());
    }

    #[test]
    fn descend_all() {
        let root = json!({"a": {"b": 1}, "c": 2});
        let all = query(&root, "$..*");
        // members: a's value {b:1}, b's value 1, c's value 2
        assert!(all.contains(&json!({"b": 1})));
        assert!(all.contains(&json!(1)));
        assert!(all.contains(&json!(2)));
    }
}
