//! Built-in dynamic variables (plan §7.3 G7/G8).
//!
//! Names beginning with `$` that match a built-in are resolved on demand
//! during substitution: `$uuid` / `$random.uuid`, `$timestamp`,
//! `$isoTimestamp`, `$randomInt` / `$random.integer(...)`, `$historyFolder`,
//! `$projectRoot`. A name that is not a recognised built-in (e.g. a JSONPath
//! template `$.clients..id`) resolves to `None` so the caller falls back to
//! the layered variable store.

use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::env::{Environment, VarValue};

/// Resolve a `$`-prefixed built-in variable name. Returns `None` when `name`
/// is not a recognised dynamic variable (the caller then does a normal scope
/// lookup — important for JSONPath templates like `$.clients..id`).
pub fn resolve_dynamic(name: &str, env: &Environment) -> Option<VarValue> {
    if !name.starts_with('$') {
        return None;
    }
    // JSONPath templates (`$.…`) are NOT dynamic built-ins.
    if name.starts_with("$.") || name == "$" {
        return None;
    }
    match name {
        "$uuid" | "$random.uuid" => Some(VarValue::Str(uuid::Uuid::new_v4().to_string())),
        "$timestamp" => Some(VarValue::Json(serde_json::Value::from(unix_secs()))),
        "$isoTimestamp" => Some(VarValue::Str(iso_now())),
        "$randomInt" => Some(random_integer(0, 999)),
        "$projectRoot" => Some(VarValue::Str(
            env.project_root().to_string_lossy().into_owned(),
        )),
        "$historyFolder" => {
            let dir = env.project_root().join(".http-history");
            let _ = std::fs::create_dir_all(&dir);
            Some(VarValue::Str(dir.to_string_lossy().into_owned()))
        }
        other => {
            // `$random.integer()` / `$random.integer(from,to)`
            if let Some(args) = other.strip_prefix("$random.integer") {
                let args = args.trim();
                let args = args
                    .strip_prefix('(')
                    .and_then(|s| s.strip_suffix(')'))
                    .unwrap_or("");
                let parts: Vec<&str> = args.split(',').map(str::trim).collect();
                return match parts.len() {
                    1 if parts[0].is_empty() => Some(random_integer(0, 999)),
                    2 => {
                        let from = parts[0].parse::<i64>().ok()?;
                        let to = parts[1].parse::<i64>().ok()?;
                        Some(random_integer(from, to))
                    }
                    _ => None,
                };
            }
            None
        }
    }
}

fn random_integer(from: i64, to: i64) -> VarValue {
    let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
    let mut rng = rand::thread_rng();
    // Inclusive range on both ends, matching JetBrains `$random.integer(a,b)`.
    let n = if hi == lo {
        lo
    } else {
        rng.gen_range(lo..=hi)
    };
    VarValue::Json(serde_json::Value::from(n))
}

fn unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn iso_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_shape() {
        let env = Environment::new();
        let v = resolve_dynamic("$random.uuid", &env).unwrap();
        let s = v.as_str();
        assert_eq!(s.len(), 36);
        assert_eq!(s.chars().filter(|c| *c == '-').count(), 4);
    }

    #[test]
    fn timestamp_is_number() {
        let env = Environment::new();
        match resolve_dynamic("$timestamp", &env).unwrap() {
            VarValue::Json(serde_json::Value::Number(_)) => {}
            other => panic!("expected JSON number, got {other:?}"),
        }
    }

    #[test]
    fn iso_timestamp_shape() {
        let env = Environment::new();
        let s = resolve_dynamic("$isoTimestamp", &env).unwrap().as_str();
        assert!(s.ends_with('Z'), "{s}");
        assert!(s.contains('T'), "{s}");
    }

    #[test]
    fn random_integer_range() {
        let env = Environment::new();
        let v = resolve_dynamic("$random.integer(5,5)", &env).unwrap();
        assert_eq!(v.as_str(), "5");
        let v = resolve_dynamic("$random.integer(1,3)", &env).unwrap();
        let n: i64 = v.as_str().parse().unwrap();
        assert!((1..=3).contains(&n));
    }

    #[test]
    fn jsonpath_not_dynamic() {
        let env = Environment::new();
        assert!(resolve_dynamic("$.clients..id", &env).is_none());
        assert!(resolve_dynamic("$", &env).is_none());
    }

    #[test]
    fn non_dollar_not_dynamic() {
        let env = Environment::new();
        assert!(resolve_dynamic("host", &env).is_none());
    }

    #[test]
    fn history_folder_created() {
        let dir = tempfile::tempdir().unwrap();
        let mut env = Environment::new();
        env.set_project_root(dir.path());
        let v = resolve_dynamic("$historyFolder", &env).unwrap();
        assert!(v.as_str().ends_with(".http-history"));
        assert!(dir.path().join(".http-history").exists());
    }
}
