//! Environment variables and substitution (spec 3.2.6 / 4.4, plan §7.3).
//!
//! An [`Environment`] is a layered variable store. Substitution scans text for
//! `{{ ... }}` references and replaces them with the resolved value,
//! preserving the literal `{{name}}` if undefined.
//!
//! Layers, highest → lowest precedence (per JetBrains HTTP Client docs):
//! `environment` (env files / explicit `.set`), then `global`
//! (`client.global.set`), then `file` (in-place `@name = value`), then
//! `request` (`request.variables.set`, JSONPath loop bindings).
//!
//! Values may be plain strings ([`VarValue::Str`]) or arbitrary JSON
//! ([`VarValue::Json`], e.g. an array set from a pre-request script). Names
//! beginning with `$` that match a built-in are resolved dynamically
//! (`$random.uuid`, `$timestamp`, …) — see [`crate::dynamic`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::dynamic::resolve_dynamic;

/// A variable value: either a plain string or a JSON value (number, bool,
/// array, object). JSON values let pre-request scripts stash structured data
/// (`request.variables.set("clients", [ … ])`) that JSONPath loops iterate.
#[derive(Debug, Clone, PartialEq)]
pub enum VarValue {
    Str(String),
    Json(serde_json::Value),
}

impl VarValue {
    /// Render this value for textual substitution.
    ///
    /// - `Str(s)`: verbatim, unless `json_quote` is set (a JSON string context
    ///   where the placeholder is NOT already wrapped in quotes) — then the
    ///   value is JSON-escaped and quoted (plan §7.3 G24).
    /// - `Json(v)`: numbers/bools/null/arrays/objects render as compact JSON.
    ///   A `Json` string renders as its inner text when the placeholder is
    ///   already inside quotes (or in a non-JSON context) and as a quoted JSON
    ///   string otherwise.
    pub fn to_substitution(&self, json_quote: bool) -> String {
        match self {
            VarValue::Str(s) => {
                if json_quote {
                    serde_json::Value::String(s.clone()).to_string()
                } else {
                    s.clone()
                }
            }
            VarValue::Json(serde_json::Value::String(s)) => {
                if json_quote {
                    serde_json::Value::String(s.clone()).to_string()
                } else {
                    s.clone()
                }
            }
            VarValue::Json(v) => v.to_string(),
        }
    }

    /// Coerce to a [`serde_json::Value`] (strings become JSON strings).
    pub fn as_json(&self) -> serde_json::Value {
        match self {
            VarValue::Str(s) => serde_json::Value::String(s.clone()),
            VarValue::Json(v) => v.clone(),
        }
    }

    /// Best-effort string view: `Str` verbatim, `Json` compact-rendered.
    pub fn as_str(&self) -> String {
        match self {
            VarValue::Str(s) => s.clone(),
            VarValue::Json(v) => match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            },
        }
    }
}

impl From<String> for VarValue {
    fn from(s: String) -> Self {
        VarValue::Str(s)
    }
}

impl From<&str> for VarValue {
    fn from(s: &str) -> Self {
        VarValue::Str(s.to_string())
    }
}

impl From<serde_json::Value> for VarValue {
    fn from(v: serde_json::Value) -> Self {
        VarValue::Json(v)
    }
}

/// Which layer a variable is written to. Reads pierce all layers in
/// precedence order regardless of where a value was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Environment,
    Global,
    File,
    Request,
}

/// Layered variable store used for `{{ ... }}` substitution.
#[derive(Debug, Clone, Default)]
pub struct Environment {
    environment: BTreeMap<String, VarValue>,
    global: BTreeMap<String, VarValue>,
    file: BTreeMap<String, VarValue>,
    request: BTreeMap<String, VarValue>,
    /// Base directory for `$projectRoot` / `$historyFolder` and relative
    /// file resolution. Defaults to the current directory.
    project_root: PathBuf,
}

impl Environment {
    pub fn new() -> Self {
        Self {
            environment: BTreeMap::new(),
            global: BTreeMap::new(),
            file: BTreeMap::new(),
            request: BTreeMap::new(),
            project_root: PathBuf::from("."),
        }
    }

    /// Set a string variable in the `environment` layer (highest precedence).
    /// Preserved from the original flat-map API.
    pub fn set<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) {
        self.environment
            .insert(key.into(), VarValue::Str(value.into()));
    }

    /// Set a variable in a specific layer with an explicit [`VarValue`].
    pub fn set_value<K: Into<String>>(&mut self, layer: Layer, key: K, value: VarValue) {
        self.layer_mut(layer).insert(key.into(), value);
    }

    /// Set a JSON variable in a specific layer.
    pub fn set_json<K: Into<String>>(&mut self, layer: Layer, key: K, value: serde_json::Value) {
        self.set_value(layer, key, VarValue::Json(value));
    }

    fn layer_mut(&mut self, layer: Layer) -> &mut BTreeMap<String, VarValue> {
        match layer {
            Layer::Environment => &mut self.environment,
            Layer::Global => &mut self.global,
            Layer::File => &mut self.file,
            Layer::Request => &mut self.request,
        }
    }

    fn layer(&self, layer: Layer) -> &BTreeMap<String, VarValue> {
        match layer {
            Layer::Environment => &self.environment,
            Layer::Global => &self.global,
            Layer::File => &self.file,
            Layer::Request => &self.request,
        }
    }

    /// Look up a variable by name across all layers (precedence order),
    /// resolving `$`-prefixed built-ins dynamically. Returns `None` if the
    /// name is neither a built-in nor defined in any layer.
    pub fn get_value(&self, name: &str) -> Option<VarValue> {
        if let Some(v) = resolve_dynamic(name, self) {
            return Some(v);
        }
        for layer in [
            Layer::Environment,
            Layer::Global,
            Layer::File,
            Layer::Request,
        ] {
            if let Some(v) = self.layer(layer).get(name) {
                return Some(v.clone());
            }
        }
        None
    }

    /// String view of a variable (backward-compatible accessor). Dynamic
    /// built-ins are resolved; JSON values are compact-rendered.
    pub fn get(&self, key: &str) -> Option<String> {
        self.get_value(key).map(|v| v.as_str())
    }

    /// Iterate the `global` layer as `(key, string)` — used to merge handler
    /// globals back for later requests.
    pub fn globals(&self) -> impl Iterator<Item = (&String, &VarValue)> {
        self.global.iter()
    }

    /// Remove all variables from the `global` layer (`client.global.clearAll`).
    pub fn clear_globals(&mut self) {
        self.global.clear();
    }

    /// Remove a single `global` variable (`client.global.clear(key)`).
    pub fn clear_global(&mut self, key: &str) {
        self.global.remove(key);
    }

    pub fn set_project_root<P: AsRef<Path>>(&mut self, root: P) {
        self.project_root = root.as_ref().to_path_buf();
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn is_empty(&self) -> bool {
        self.environment.is_empty()
            && self.global.is_empty()
            && self.file.is_empty()
            && self.request.is_empty()
    }

    /// Build the JSON object that JSONPath loop templates (`{{$.clients..id}}`)
    /// are evaluated against (plan §7.4 G10). `$` is the combined variable
    /// scope; higher-precedence layers override lower ones
    /// (request < file < global < environment).
    pub fn variables_root(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for layer in [
            Layer::Request,
            Layer::File,
            Layer::Global,
            Layer::Environment,
        ] {
            for (k, v) in self.layer(layer) {
                map.insert(k.clone(), v.as_json());
            }
        }
        serde_json::Value::Object(map)
    }
}

/// Substitute `{{ ... }}` references in `text` using `env` (non-JSON context:
/// string values are inserted verbatim, never quoted). Undefined references
/// are preserved verbatim, including any inner whitespace.
pub fn substitute(text: &str, env: &Environment) -> String {
    substitute_impl(text, env, false)
}

/// JSON-aware substitution (plan §7.3 G24): a `Str` value inserted at a
/// placeholder that is NOT already wrapped in double quotes is JSON-escaped
/// and quoted, so `"id": {{$random.uuid}}` yields valid JSON. Numbers, bools,
/// arrays and objects render bare/compact regardless.
pub fn substitute_json(text: &str, env: &Environment) -> String {
    substitute_impl(text, env, true)
}

fn substitute_impl(text: &str, env: &Environment, json_context: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.char_indices().collect::<Vec<_>>();
    let mut i = 0;
    while i < bytes.len() {
        let (_, ch) = bytes[i];
        if ch == '{' && i + 1 < bytes.len() && bytes[i + 1].1 == '{' {
            if let Some((end, name)) = scan_env_var(&bytes, i) {
                match env.get_value(&name) {
                    Some(value) => {
                        // Determine whether this placeholder is already inside
                        // a JSON string literal (adjacent `"`), in which case
                        // we must NOT add quotes.
                        let json_quote = if json_context {
                            let prev_quote = i > 0 && bytes[i - 1].1 == '"';
                            let next_idx = end + 1;
                            let next_quote = next_idx < bytes.len() && bytes[next_idx].1 == '"';
                            !(prev_quote || next_quote)
                        } else {
                            false
                        };
                        out.push_str(&value.to_substitution(json_quote));
                    }
                    None => {
                        // Preserve the original span (incl. whitespace) verbatim.
                        out.push_str(
                            &text[bytes[i].0..bytes[end].0 + bytes[end].1.len_utf8()],
                        );
                    }
                }
                i = end + 1;
                continue;
            }
        }
        out.push(ch);
        i += 1;
    }
    out
}

/// Identifier characters inside `{{ ... }}`. Extended (plan §7.3 G7) beyond
/// the original `[A-Za-z0-9_-]` to cover dynamic variables (`$random.uuid`,
/// `$random.integer(1,100)`) and JSONPath templates (`$.clients..id`).
/// Braces are excluded so the `}}` terminator is unambiguous.
fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '-' | '_' | '.' | '$' | '(' | ')' | '[' | ']' | '\'' | '"' | ',' | '=' | ' ' | ':'
        )
}

/// If `bytes[start]` begins a `{{ [ws] identifier [ws] }}` sequence, return
/// the index of the closing `}` and the trimmed identifier.
fn scan_env_var(bytes: &[(usize, char)], start: usize) -> Option<(usize, String)> {
    let mut i = start + 2;
    while i < bytes.len() && bytes[i].1.is_whitespace() {
        i += 1;
    }
    let id_start = i;
    while i < bytes.len() && is_ident_char(bytes[i].1) {
        i += 1;
    }
    let id_end = i;
    if id_end == id_start {
        return None;
    }
    while i < bytes.len() && bytes[i].1.is_whitespace() {
        i += 1;
    }
    if i + 1 < bytes.len() && bytes[i].1 == '}' && bytes[i + 1].1 == '}' {
        let name: String = bytes[id_start..id_end]
            .iter()
            .map(|(_, c)| *c)
            .collect::<String>()
            .trim()
            .to_string();
        if name.is_empty() {
            return None;
        }
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

/// Errors produced while loading JetBrains environment files.
#[derive(Debug, thiserror::Error)]
pub enum EnvLoadError {
    #[error("env file {path}: {reason}")]
    Io { path: PathBuf, reason: String },
    #[error("env file {path}: {reason}")]
    Parse { path: PathBuf, reason: String },
    #[error("environment {name:?} not found in {path}")]
    NotFound { name: String, path: PathBuf },
}

/// Load a named environment from JetBrains env files (plan §7.3 G21).
///
/// Resolution:
/// - base file: `env_file` if given, else `<cwd>/http-client.env.json`;
/// - private file: the base file's sibling with `.private.env.json`
///   (`http-client.private.env.json`) — merged *after* the base file so its
///   values win, and ignored when absent;
/// - a `"$shared"` object in either file applies to every environment.
///
/// `env_name == None` yields an empty environment (still rooted at `cwd`).
/// JSON strings become [`VarValue::Str`]; numbers/bools/arrays/objects become
/// [`VarValue::Json`] so they can be substituted bare into JSON bodies.
pub fn load_env_files(
    env_name: Option<&str>,
    env_file: Option<&Path>,
    cwd: &Path,
) -> Result<Environment, EnvLoadError> {
    let mut env = Environment::new();
    env.set_project_root(cwd);
    let Some(name) = env_name else {
        return Ok(env);
    };
    let base = env_file
        .map(Path::to_path_buf)
        .unwrap_or_else(|| cwd.join("http-client.env.json"));
    let private = private_sibling(&base);

    let mut found = false;
    // Order matters: base first, private second (private overrides).
    for (path, required) in [(base.clone(), true), (private, false)] {
        if !path.exists() {
            if required {
                return Err(EnvLoadError::Io {
                    reason: "file not found".to_string(),
                    path,
                });
            }
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| EnvLoadError::Io {
            reason: e.to_string(),
            path: path.clone(),
        })?;
        let parsed: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| EnvLoadError::Parse {
                reason: e.to_string(),
                path: path.clone(),
            })?;
        let obj = parsed.as_object().ok_or_else(|| EnvLoadError::Parse {
            reason: "expected a JSON object of environments".to_string(),
            path: path.clone(),
        })?;
        if let Some(shared) = obj.get("$shared").and_then(|v| v.as_object()) {
            insert_vars(&mut env, shared);
        }
        if let Some(vars) = obj.get(name).and_then(|v| v.as_object()) {
            insert_vars(&mut env, vars);
            found = true;
        }
    }
    if !found {
        return Err(EnvLoadError::NotFound {
            name: name.to_string(),
            path: base,
        });
    }
    Ok(env)
}

fn insert_vars(env: &mut Environment, vars: &serde_json::Map<String, serde_json::Value>) {
    for (k, v) in vars {
        match v {
            serde_json::Value::String(s) => {
                env.set_value(Layer::Environment, k.clone(), VarValue::Str(s.clone()));
            }
            other => env.set_json(Layer::Environment, k.clone(), other.clone()),
        }
    }
}

/// `http-client.env.json` → `http-client.private.env.json`; for a custom
/// `foo.env.json` → `foo.private.env.json`; otherwise `foo.json` →
/// `foo.private.json`.
fn private_sibling(base: &Path) -> PathBuf {
    let name = base
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let private = if let Some(stem) = name.strip_suffix(".env.json") {
        format!("{stem}.private.env.json")
    } else if let Some(stem) = name.strip_suffix(".json") {
        format!("{stem}.private.json")
    } else {
        return base.to_path_buf();
    };
    base.with_file_name(private)
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
    fn dynamic_variable_scan() {
        // `$random.uuid` and `$.clients..id` must scan as identifiers.
        let refs = collect_references("{{$random.uuid}} {{$.clients..id}} {{$random.integer(1,100)}}");
        assert_eq!(
            refs,
            vec![
                "$random.uuid".to_string(),
                "$.clients..id".to_string(),
                "$random.integer(1,100)".to_string()
            ]
        );
    }

    #[test]
    fn json_autoquote_string_value() {
        let mut env = Environment::new();
        env.set_value(Layer::Request, "id", VarValue::Str("abc".into()));
        // Not already quoted → quoted.
        assert_eq!(substitute_json(r#"{"id": {{id}}}"#, &env), r#"{"id": "abc"}"#);
        // Already quoted → not double-quoted.
        assert_eq!(substitute_json(r#"{"id": "{{id}}"}"#, &env), r#"{"id": "abc"}"#);
        // Non-JSON substitution never quotes.
        assert_eq!(substitute("{{id}}", &env), "abc");
    }

    #[test]
    fn json_number_bare() {
        let mut env = Environment::new();
        env.set_json(Layer::Request, "n", serde_json::json!(42));
        assert_eq!(substitute_json(r#"{"n": {{n}}}"#, &env), r#"{"n": 42}"#);
        // In a string context a JSON number renders bare too (JetBrains inserts numbers as-is).
        assert_eq!(substitute("{{n}}", &env), "42");
    }

    #[test]
    fn json_string_value_inside_quotes_is_not_double_quoted() {
        let mut env = Environment::new();
        env.set_json(Layer::Request, "name", serde_json::json!("George"));
        // Quoted placeholder must not gain a second pair of quotes.
        assert_eq!(substitute_json(r#"{"n": "{{name}}"}"#, &env), r#"{"n": "George"}"#);
        // Bare placeholder gets auto-quoted.
        assert_eq!(substitute_json(r#"{"n": {{name}}}"#, &env), r#"{"n": "George"}"#);
        // Non-JSON substitution renders the inner text.
        assert_eq!(substitute("/x/{{name}}", &env), "/x/George");
    }

    #[test]
    fn layer_precedence() {
        let mut env = Environment::new();
        env.set_value(Layer::Request, "k", VarValue::Str("req".into()));
        env.set_value(Layer::Environment, "k", VarValue::Str("env".into()));
        assert_eq!(env.get("k").as_deref(), Some("env"));
    }

    #[test]
    fn load_env_files_merges_private_and_shared() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("http-client.env.json"),
            r#"{
                "$shared": { "shared_key": "s" },
                "dev": { "host": "https://httpbin.org", "show_env": 1,
                         "clients": [{ "id": 1 }, { "id": 2 }] }
            }"#,
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("http-client.private.env.json"),
            r#"{ "dev": { "secret": "hidden" } }"#,
        )
        .unwrap();

        let env = load_env_files(Some("dev"), None, tmp.path()).unwrap();
        assert_eq!(env.get("host").as_deref(), Some("https://httpbin.org"));
        assert_eq!(env.get("shared_key").as_deref(), Some("s"));
        assert_eq!(env.get("secret").as_deref(), Some("hidden"));
        // Non-string JSON keeps its shape for bare insertion into JSON bodies.
        assert_eq!(env.get_value("show_env"), Some(VarValue::Json(serde_json::json!(1))));
        assert_eq!(substitute_json(r#"{"n": {{show_env}}}"#, &env), r#"{"n": 1}"#);
        // Arrays round-trip for JSONPath loops.
        match env.get_value("clients").unwrap() {
            VarValue::Json(serde_json::Value::Array(a)) => assert_eq!(a.len(), 2),
            other => panic!("expected JSON array, got {other:?}"),
        }
        assert_eq!(env.project_root(), tmp.path());
    }

    #[test]
    fn load_env_files_private_overrides_base() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("http-client.env.json"),
            r#"{ "dev": { "token": "public" } }"#,
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("http-client.private.env.json"),
            r#"{ "dev": { "token": "private" } }"#,
        )
        .unwrap();
        let env = load_env_files(Some("dev"), None, tmp.path()).unwrap();
        assert_eq!(env.get("token").as_deref(), Some("private"));
    }

    #[test]
    fn load_env_files_unknown_environment_errors() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("http-client.env.json"),
            r#"{ "dev": {} }"#,
        )
        .unwrap();
        let err = load_env_files(Some("prod"), None, tmp.path()).unwrap_err();
        assert!(matches!(err, EnvLoadError::NotFound { .. }));
    }

    #[test]
    fn load_env_files_missing_base_errors_but_private_is_optional() {
        let tmp = tempfile::tempdir().unwrap();
        let err = load_env_files(Some("dev"), None, tmp.path()).unwrap_err();
        assert!(matches!(err, EnvLoadError::Io { .. }));
    }

    #[test]
    fn load_env_files_without_name_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let env = load_env_files(None, None, tmp.path()).unwrap();
        assert!(env.is_empty());
    }

    #[test]
    fn private_sibling_naming() {
        assert_eq!(
            private_sibling(Path::new("/x/http-client.env.json")).file_name().unwrap(),
            "http-client.private.env.json"
        );
        assert_eq!(
            private_sibling(Path::new("/x/custom.env.json")).file_name().unwrap(),
            "custom.private.env.json"
        );
    }
}
