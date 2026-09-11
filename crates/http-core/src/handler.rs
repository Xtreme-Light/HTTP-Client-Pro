//! Response / pre-request handler script engine (spec 4.5, plan §7.5).
//!
//! Runs an ES5.1 JavaScript snippet in a QuickJS sandbox (rquickjs) exposing
//! the JetBrains HTTP Client script API:
//!
//! - `client.test(name, fn)` / `client.assert(cond, msg)` / `client.exit()`
//! - `client.global.set/get/clear/clearAll/all`, `client.log(...)`
//! - `response.status/body/headers.value|valueOf|valuesOf/contentType`
//! - `request.variables.set/get`, `request.environment.get`,
//!   `request.body.tryGetSubstituted()`, `request.iteration()`,
//!   `request.templateValue(i)`, `request.method/url/headers`
//! - `crypto.<algo>()` / `crypto.hmac.<algo>()` hash chains
//! - `jsonPath(obj, expr)`
//! - `import {x} from "./module"` (resolved on the Rust side before eval)
//!
//! The implementation splits in two layers: a small set of *native* functions
//! (`__native.*`) that only exchange primitives and JSON strings, plus a JS
//! *glue* prelude that builds the ergonomic object/chain API on top of them.
//! Script exceptions and assertion failures are captured as [`Diagnostic`]s —
//! they never bubble to the caller, so subsequent requests still execute.
//!
//! Globals set via `client.global.set` persist across runs on the same
//! runtime, so an auth handler can stash a token that a later request
//! references via `{{token}}` (Phase 3 / TDD P3-6).

use rquickjs::{
    context::Ctx, function::Rest, prelude::Coerced, CaughtError, Context, Function, IntoJs, Object,
    Runtime, Value,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::dispatch::DispatchResponse;
use crate::env::{Environment, Layer, VarValue};
use crate::error::{CoreError, Diagnostic, ErrorKind, Result, Span};

/// Default per-script timeout: 30 seconds. Guards against runaway scripts
/// (e.g. `while(true)`) hanging the dispatcher thread. The spec is silent
/// on handler timeouts; this is a project policy decision (plan §10 风险).
pub const DEFAULT_HANDLER_TIMEOUT: Duration = Duration::from_secs(30);

/// Value thrown by `client.exit()`. Recognised by the eval error handler so a
/// deliberate exit is not reported as a script error.
const EXIT_SENTINEL: &str = "__http_client_exit__";

/// Outcome of one `client.test(name, fn)` call.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    /// Failure message (assertion text or exception message), if any.
    pub message: Option<String>,
}

/// A variable write requested by a script (`request.variables.set`). The
/// runner applies these to the [`Environment`] after the script finishes.
#[derive(Debug, Clone, PartialEq)]
pub struct VarWrite {
    pub layer: Layer,
    pub key: String,
    pub value: VarValue,
}

/// Request-side data exposed to scripts as the `request` object.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RequestInfo {
    pub method: String,
    /// Fully substituted request URL.
    pub url: String,
    pub headers: Vec<(String, String)>,
    /// Substituted body text (`request.body.tryGetSubstituted()`); `None` when
    /// there is no body or substitution failed.
    pub body: Option<String>,
    /// 0-based loop iteration index (`request.iteration()`).
    pub iteration: usize,
    /// Values bound for the request's `{{$.path}}` templates, in order of
    /// appearance (`request.templateValue(i)`).
    pub template_values: Vec<serde_json::Value>,
}

/// Everything a single script run may read.
pub struct HandlerInput<'a> {
    /// `None` for pre-request scripts, where `response` is `undefined`.
    pub response: Option<&'a DispatchResponse>,
    pub request: &'a RequestInfo,
    pub env: &'a Environment,
    /// Directory used to resolve `import … from "./module"` specifiers.
    pub base_dir: Option<&'a Path>,
}

/// Per-run state shared between Rust and the JS sandbox via `Rc<RefCell>`.
/// A single `HandlerRuntime` can run multiple scripts; `globals` persist
/// across runs, everything else accumulates until the caller drains it.
#[derive(Default)]
struct HandlerState {
    globals: HashMap<String, String>,
    logs: Vec<String>,
    diagnostics: Vec<Diagnostic>,
    tests: Vec<TestResult>,
    var_writes: Vec<VarWrite>,
    exit: bool,
}

/// Immutable per-run inputs, shared with the native closures.
struct RunData {
    request: RequestInfo,
    env: Environment,
}

/// Stateful ES5.1 handler runtime (spec 4.5).
///
/// Cheap to construct; the heavy QuickJS `Runtime` is created per run so a
/// no-op request (no `<`/`>` handler) pays nothing.
pub struct HandlerRuntime {
    state: Rc<RefCell<HandlerState>>,
    /// Per-script execution timeout. Override with [`Self::with_timeout`].
    timeout: Duration,
}

impl Default for HandlerRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerRuntime {
    /// Empty runtime with no globals, logs, or diagnostics, and the default
    /// per-script timeout ([`DEFAULT_HANDLER_TIMEOUT`]).
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(HandlerState::default())),
            timeout: DEFAULT_HANDLER_TIMEOUT,
        }
    }

    /// Set the per-script execution timeout. Applies to subsequent runs. A
    /// timeout of `Duration::ZERO` disables the interrupt handler (scripts may
    /// run indefinitely).
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Run a response handler `script` against `response`, mutating this
    /// runtime's state. JS exceptions are caught and recorded as a
    /// [`Diagnostic`] (kind `Handler`) — this method only returns `Err` for
    /// catastrophic QuickJS init failures, not for script errors.
    pub fn run(&mut self, script: &str, response: &DispatchResponse) -> Result<()> {
        let request = RequestInfo::default();
        let env = Environment::new();
        self.run_with(
            script,
            HandlerInput {
                response: Some(response),
                request: &request,
                env: &env,
                base_dir: None,
            },
        )
    }

    /// Run a pre-request script (`< {% … %}`): same sandbox, but `response`
    /// is `undefined`.
    pub fn run_pre_request(
        &mut self,
        script: &str,
        request: &RequestInfo,
        env: &Environment,
        base_dir: Option<&Path>,
    ) -> Result<()> {
        self.run_with(
            script,
            HandlerInput {
                response: None,
                request,
                env,
                base_dir,
            },
        )
    }

    /// Full entry point: run `script` with the given inputs.
    pub fn run_with(&mut self, script: &str, input: HandlerInput<'_>) -> Result<()> {
        let runtime = Runtime::new().map_err(|e| handler_err("runtime init", e))?;
        let ctx = Context::full(&runtime).map_err(|e| handler_err("context init", e))?;

        // Install the interrupt handler BEFORE evaluating anything so runaway
        // loops (e.g. `while(true)`) are killed rather than hanging the
        // dispatcher. Returning `true` raises an uncatchable JS exception once
        // `timeout` elapses since `start`.
        if self.timeout > Duration::ZERO {
            let start = Instant::now();
            let timeout = self.timeout;
            runtime.set_interrupt_handler(Some(Box::new(move || start.elapsed() >= timeout)));
        }

        let state = self.state.clone();
        let run_data = Rc::new(RunData {
            request: input.request.clone(),
            env: input.env.clone(),
        });
        ctx.with(|ctx| -> Result<()> {
            build_native(&ctx, state.clone(), run_data.clone())?;
            build_request_info(&ctx, &run_data.request)?;
            build_client(&ctx, state.clone())?;
            build_assert(&ctx, state.clone())?;
            build_response(&ctx, input.response, state.clone())?;
            eval_capture(&ctx, &state, GLUE_JS);

            // `import … from "./mod"` is not ES5.1: resolve modules on the Rust
            // side, evaluate them first (their declarations become globals),
            // then evaluate the script with the import lines removed.
            let imports = collect_imports(script, input.base_dir);
            for diag in imports.diagnostics {
                state.borrow_mut().diagnostics.push(diag);
            }
            for (_path, source) in &imports.modules {
                eval_capture(&ctx, &state, source);
            }
            eval_capture(&ctx, &state, &imports.script);
            Ok(())
        })
    }

    /// Load a script from `path` (spec 3.2.4 file-handler form `> file.js`)
    /// and run it as a response handler. Missing file or read error returns
    /// `Err` of kind `Io`. Imports resolve relative to the script's directory.
    pub fn run_file(&mut self, path: &str, response: &DispatchResponse) -> Result<()> {
        let script = read_script(path)?;
        let request = RequestInfo::default();
        let env = Environment::new();
        self.run_with(
            &script,
            HandlerInput {
                response: Some(response),
                request: &request,
                env: &env,
                base_dir: Path::new(path).parent(),
            },
        )
    }

    /// Load a script from `path` and run it as a pre-request script.
    pub fn run_file_pre_request(
        &mut self,
        path: &str,
        request: &RequestInfo,
        env: &Environment,
    ) -> Result<()> {
        let script = read_script(path)?;
        self.run_with(
            &script,
            HandlerInput {
                response: None,
                request,
                env,
                base_dir: Path::new(path).parent(),
            },
        )
    }

    /// Globals set via `client.global.set` during prior runs. The caller can
    /// merge these into an [`Environment`] so later requests can substitute
    /// `{{key}}` references (TDD P3-6).
    pub fn globals(&self) -> HashMap<String, String> {
        self.state.borrow().globals.clone()
    }

    /// `client.log(...)` output accumulated so far.
    pub fn logs(&self) -> Vec<String> {
        self.state.borrow().logs.clone()
    }

    /// Diagnostics accumulated from assertion failures and JS exceptions.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        self.state.borrow().diagnostics.clone()
    }

    /// `client.test(...)` outcomes accumulated so far.
    pub fn tests(&self) -> Vec<TestResult> {
        self.state.borrow().tests.clone()
    }

    /// Drain the recorded test outcomes.
    pub fn take_tests(&mut self) -> Vec<TestResult> {
        std::mem::take(&mut self.state.borrow_mut().tests)
    }

    /// Drain the captured log lines.
    pub fn take_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.state.borrow_mut().logs)
    }

    /// Drain the recorded diagnostics.
    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.state.borrow_mut().diagnostics)
    }

    /// Drain the variable writes requested by scripts.
    pub fn take_var_writes(&mut self) -> Vec<VarWrite> {
        std::mem::take(&mut self.state.borrow_mut().var_writes)
    }

    /// `true` once a script called `client.exit()`; the runner stops the file.
    pub fn exit_requested(&self) -> bool {
        self.state.borrow().exit
    }

    /// Drop all accumulated globals (`client.global.clearAll` from Rust).
    pub fn clear_globals(&mut self) {
        self.state.borrow_mut().globals.clear();
    }
}

fn read_script(path: &str) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| {
        CoreError::new(
            ErrorKind::Io,
            Span::new(0, 0),
            format!("failed to read handler `{path}`: {e}"),
        )
    })
}

fn handler_err(prefix: &str, e: rquickjs::Error) -> CoreError {
    CoreError::new(ErrorKind::Handler, Span::new(0, 0), format!("{prefix}: {e}"))
}

fn diag(message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        kind: ErrorKind::Handler,
        span: Span::new(0, 0),
        message: message.into(),
    }
}

/// Evaluate `source`, converting JS exceptions into `Handler` diagnostics.
/// The `client.exit()` sentinel is recognised and silently swallowed.
fn eval_capture(ctx: &Ctx, state: &Rc<RefCell<HandlerState>>, source: &str) {
    let eval_result: rquickjs::Result<Value> = ctx.eval(source);
    if let Err(e) = eval_result {
        let caught = CaughtError::from_error(ctx, e);
        let text = format!("{caught}");
        if text.contains(EXIT_SENTINEL) {
            return;
        }
        state
            .borrow_mut()
            .diagnostics
            .push(diag(format!("script error: {text}")));
    }
}

// ---------------------------------------------------------------------------
// Native layer (`__native.*`) — primitives and JSON strings only
// ---------------------------------------------------------------------------

fn build_native(
    ctx: &Ctx,
    state: Rc<RefCell<HandlerState>>,
    run_data: Rc<RunData>,
) -> Result<()> {
    let native = Object::new(ctx.clone()).map_err(|e| handler_err("native obj", e))?;

    // --- logs / diagnostics / tests -------------------------------------
    let st = state.clone();
    install(
        ctx,
        &native,
        "assertFailed",
        Function::new(ctx.clone(), move |msg: String| {
            st.borrow_mut().diagnostics.push(diag(msg));
        })
        .map_err(|e| handler_err("assertFailed", e))?,
    )?;

    let st = state.clone();
    install(
        ctx,
        &native,
        "endTest",
        Function::new(
            ctx.clone(),
            move |name: String, passed: bool, message: String| {
                st.borrow_mut().tests.push(TestResult {
                    name,
                    passed,
                    message: if message.is_empty() {
                        None
                    } else {
                        Some(message)
                    },
                });
            },
        )
        .map_err(|e| handler_err("endTest", e))?,
    )?;

    let st = state.clone();
    install(
        ctx,
        &native,
        "markExit",
        Function::new(ctx.clone(), move || {
            st.borrow_mut().exit = true;
        })
        .map_err(|e| handler_err("markExit", e))?,
    )?;

    // --- client.global ---------------------------------------------------
    let st = state.clone();
    install(
        ctx,
        &native,
        "globalSet",
        Function::new(ctx.clone(), move |key: String, value: String| {
            st.borrow_mut().globals.insert(key, value);
        })
        .map_err(|e| handler_err("globalSet", e))?,
    )?;

    let st = state.clone();
    install(
        ctx,
        &native,
        "globalGet",
        Function::new(ctx.clone(), move |key: String| -> Option<String> {
            st.borrow().globals.get(&key).cloned()
        })
        .map_err(|e| handler_err("globalGet", e))?,
    )?;

    let st = state.clone();
    install(
        ctx,
        &native,
        "globalClear",
        Function::new(ctx.clone(), move |key: String| {
            st.borrow_mut().globals.remove(&key);
        })
        .map_err(|e| handler_err("globalClear", e))?,
    )?;

    let st = state.clone();
    install(
        ctx,
        &native,
        "globalClearAll",
        Function::new(ctx.clone(), move || {
            st.borrow_mut().globals.clear();
        })
        .map_err(|e| handler_err("globalClearAll", e))?,
    )?;

    let st = state.clone();
    install(
        ctx,
        &native,
        "globalAll",
        Function::new(ctx.clone(), move || -> String {
            let map: serde_json::Map<String, serde_json::Value> = st
                .borrow()
                .globals
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            serde_json::Value::Object(map).to_string()
        })
        .map_err(|e| handler_err("globalAll", e))?,
    )?;

    // --- request.variables / request.environment -------------------------
    // Values cross the boundary as JSON text; the glue layer parses them back
    // into real JS values so `request.variables.get("clients")[0]` works.
    let st = state.clone();
    let rd = run_data.clone();
    install(
        ctx,
        &native,
        "varGet",
        Function::new(ctx.clone(), move |key: String| -> Option<String> {
            // A value written earlier in the same run wins over the snapshot.
            if let Some(w) = st
                .borrow()
                .var_writes
                .iter()
                .rev()
                .find(|w| w.key == key)
                .cloned()
            {
                return Some(w.value.as_json().to_string());
            }
            rd.env.get_value(&key).map(|v| v.as_json().to_string())
        })
        .map_err(|e| handler_err("varGet", e))?,
    )?;

    let st = state.clone();
    install(
        ctx,
        &native,
        "varSet",
        Function::new(ctx.clone(), move |key: String, json: String| {
            let value = match serde_json::from_str::<serde_json::Value>(&json) {
                Ok(v) => VarValue::Json(v),
                Err(_) => VarValue::Str(json),
            };
            st.borrow_mut().var_writes.push(VarWrite {
                layer: Layer::Request,
                key,
                value,
            });
        })
        .map_err(|e| handler_err("varSet", e))?,
    )?;

    let rd = run_data.clone();
    install(
        ctx,
        &native,
        "envGet",
        Function::new(ctx.clone(), move |key: String| -> Option<String> {
            rd.env.get_value(&key).map(|v| v.as_json().to_string())
        })
        .map_err(|e| handler_err("envGet", e))?,
    )?;

    let rd = run_data.clone();
    install(
        ctx,
        &native,
        "requestBody",
        Function::new(ctx.clone(), move || -> Option<String> {
            rd.request.body.clone()
        })
        .map_err(|e| handler_err("requestBody", e))?,
    )?;

    let rd = run_data.clone();
    install(
        ctx,
        &native,
        "iteration",
        Function::new(ctx.clone(), move || -> usize { rd.request.iteration })
            .map_err(|e| handler_err("iteration", e))?,
    )?;

    let rd = run_data.clone();
    install(
        ctx,
        &native,
        "templateValue",
        Function::new(ctx.clone(), move |i: i64| -> Option<String> {
            if i < 0 {
                return None;
            }
            rd.request
                .template_values
                .get(i as usize)
                .map(|v| v.to_string())
        })
        .map_err(|e| handler_err("templateValue", e))?,
    )?;

    // --- crypto ----------------------------------------------------------
    install(
        ctx,
        &native,
        "hash",
        Function::new(ctx.clone(), |algo: String, text: String| -> Option<String> {
            hash_hex(&algo, text.as_bytes())
        })
        .map_err(|e| handler_err("hash", e))?,
    )?;

    install(
        ctx,
        &native,
        "hmacHash",
        Function::new(
            ctx.clone(),
            |algo: String, secret: String, text: String| -> Option<String> {
                hmac_hex(&algo, secret.as_bytes(), text.as_bytes())
            },
        )
        .map_err(|e| handler_err("hmacHash", e))?,
    )?;

    install(
        ctx,
        &native,
        "hexToBase64",
        Function::new(ctx.clone(), |h: String| -> Option<String> {
            use base64::Engine;
            let bytes = hex::decode(h.trim()).ok()?;
            Some(base64::engine::general_purpose::STANDARD.encode(bytes))
        })
        .map_err(|e| handler_err("hexToBase64", e))?,
    )?;

    install(
        ctx,
        &native,
        "base64ToText",
        Function::new(ctx.clone(), |b: String| -> Option<String> {
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b.trim())
                .ok()?;
            Some(String::from_utf8_lossy(&bytes).into_owned())
        })
        .map_err(|e| handler_err("base64ToText", e))?,
    )?;

    // --- jsonPath --------------------------------------------------------
    install(
        ctx,
        &native,
        "jsonPath",
        Function::new(
            ctx.clone(),
            |json: String, expr: String| -> Option<String> {
                let root: serde_json::Value = serde_json::from_str(&json).ok()?;
                let matches = crate::jsonpath::query(&root, &expr);
                match matches.len() {
                    0 => None,
                    1 => Some(matches[0].to_string()),
                    _ => Some(serde_json::Value::Array(matches).to_string()),
                }
            },
        )
        .map_err(|e| handler_err("jsonPath", e))?,
    )?;

    ctx.globals()
        .set("__native", native)
        .map_err(|e| handler_err("__native install", e))?;
    Ok(())
}

fn install<'js>(ctx: &Ctx<'js>, obj: &Object<'js>, name: &str, f: Function<'js>) -> Result<()> {
    let _ = ctx;
    obj.set(name, f)
        .map_err(|e| handler_err(&format!("{name} install"), e))
}

// ---------------------------------------------------------------------------
// Data objects installed from Rust
// ---------------------------------------------------------------------------

/// `__requestInfo` — the static request facts the glue layer copies onto the
/// `request` object.
fn build_request_info(ctx: &Ctx, request: &RequestInfo) -> Result<()> {
    let info = Object::new(ctx.clone()).map_err(|e| handler_err("requestInfo obj", e))?;
    info.set("method", request.method.clone())
        .map_err(|e| handler_err("requestInfo.method", e))?;
    info.set("url", request.url.clone())
        .map_err(|e| handler_err("requestInfo.url", e))?;
    let headers = Object::new(ctx.clone()).map_err(|e| handler_err("requestInfo headers", e))?;
    for (name, value) in &request.headers {
        // First occurrence wins, mirroring a single-valued header view.
        if headers.get::<_, Option<String>>(name.as_str()).ok().flatten().is_none() {
            headers
                .set(name.as_str(), value.clone())
                .map_err(|e| handler_err("requestInfo header set", e))?;
        }
    }
    info.set("headers", headers)
        .map_err(|e| handler_err("requestInfo.headers", e))?;
    ctx.globals()
        .set("__requestInfo", info)
        .map_err(|e| handler_err("__requestInfo install", e))?;
    Ok(())
}

/// Install `client.log(...)` — the rest of `client` is filled in by the glue.
fn build_client(ctx: &Ctx, state: Rc<RefCell<HandlerState>>) -> Result<()> {
    let client = Object::new(ctx.clone()).map_err(|e| handler_err("client obj", e))?;
    let log_fn = Function::new(ctx.clone(), move |args: Rest<Coerced<String>>| {
        let joined = args
            .0
            .iter()
            .map(|c| c.0.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        state.borrow_mut().logs.push(joined);
    })
    .map_err(|e| handler_err("client.log", e))?;
    client
        .set("log", log_fn)
        .map_err(|e| handler_err("client.log install", e))?;
    ctx.globals()
        .set("client", client)
        .map_err(|e| handler_err("client install", e))?;
    Ok(())
}

/// Install the global `assert(cond, msg)` function (spec 4.5 — `assert` is a
/// top-level name, distinct from `client.assert`). On failure it records a
/// diagnostic and continues execution (does NOT throw).
fn build_assert(ctx: &Ctx, state: Rc<RefCell<HandlerState>>) -> Result<()> {
    let assert_fn = Function::new(ctx.clone(), move |cond: bool, msg: String| {
        if !cond {
            state.borrow_mut().diagnostics.push(diag(msg));
        }
    })
    .map_err(|e| handler_err("assert", e))?;
    ctx.globals()
        .set("assert", assert_fn)
        .map_err(|e| handler_err("assert install", e))?;
    Ok(())
}

/// Install the `response` object, or `undefined` for pre-request scripts.
fn build_response(
    ctx: &Ctx,
    response: Option<&DispatchResponse>,
    state: Rc<RefCell<HandlerState>>,
) -> Result<()> {
    let Some(response) = response else {
        ctx.globals()
            .set("response", Value::new_undefined(ctx.clone()))
            .map_err(|e| handler_err("response install", e))?;
        return Ok(());
    };

    let resp_obj = Object::new(ctx.clone()).map_err(|e| handler_err("response obj", e))?;
    resp_obj
        .set("status", response.status)
        .map_err(|e| handler_err("response.status", e))?;

    // body: parsed JSON when the Content-Type says so, else a plain string.
    let content_type = response.header("Content-Type").unwrap_or("");
    let body_str = String::from_utf8_lossy(&response.body).into_owned();
    let body_value: Value = if content_type.to_ascii_lowercase().contains("json") {
        let json_obj: Object = ctx
            .globals()
            .get("JSON")
            .map_err(|e| handler_err("JSON lookup", e))?;
        let parse_fn: Function = json_obj
            .get("parse")
            .map_err(|e| handler_err("JSON.parse lookup", e))?;
        let body_js_str = body_str
            .clone()
            .into_js(ctx)
            .map_err(|e| handler_err("body into_js", e))?;
        parse_fn.call((body_js_str,)).unwrap_or_else(|e| {
            // Surface a parse error as a diagnostic instead of bubbling.
            state
                .borrow_mut()
                .diagnostics
                .push(diag(format!("JSON.parse failed: {e}")));
            Value::new_undefined(ctx.clone())
        })
    } else {
        body_str
            .into_js(ctx)
            .map_err(|e| handler_err("body string", e))?
    };
    resp_obj
        .set("body", body_value)
        .map_err(|e| handler_err("response.body install", e))?;

    // headers: value/valueOf (first match, case-insensitive), valuesOf (all).
    let headers_obj = Object::new(ctx.clone()).map_err(|e| handler_err("headers obj", e))?;
    let snapshot: Vec<(String, String)> = response.headers.clone();
    for name in ["value", "valueOf"] {
        let snap = snapshot.clone();
        let f = Function::new(ctx.clone(), move |key: String| -> Option<String> {
            snap.iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(&key))
                .map(|(_, v)| v.clone())
        })
        .map_err(|e| handler_err("headers.value", e))?;
        headers_obj
            .set(name, f)
            .map_err(|e| handler_err("headers.value install", e))?;
    }
    let snap = snapshot.clone();
    let values_of = Function::new(ctx.clone(), move |key: String| -> Vec<String> {
        snap.iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(&key))
            .map(|(_, v)| v.clone())
            .collect()
    })
    .map_err(|e| handler_err("headers.valuesOf", e))?;
    headers_obj
        .set("valuesOf", values_of)
        .map_err(|e| handler_err("headers.valuesOf install", e))?;
    resp_obj
        .set("headers", headers_obj)
        .map_err(|e| handler_err("response.headers install", e))?;

    // contentType: { mimeType, charset, boundary }
    let ct = parse_content_type(content_type);
    let ct_obj = Object::new(ctx.clone()).map_err(|e| handler_err("contentType obj", e))?;
    for (key, value) in [
        ("mimeType", ct.mime_type),
        ("charset", ct.charset),
        ("boundary", ct.boundary),
    ] {
        match value {
            Some(v) => ct_obj
                .set(key, v)
                .map_err(|e| handler_err("contentType field", e))?,
            None => ct_obj
                .set(key, Value::new_undefined(ctx.clone()))
                .map_err(|e| handler_err("contentType field", e))?,
        }
    }
    resp_obj
        .set("contentType", ct_obj)
        .map_err(|e| handler_err("response.contentType install", e))?;

    ctx.globals()
        .set("response", resp_obj)
        .map_err(|e| handler_err("response install", e))?;
    Ok(())
}

/// Parsed `Content-Type` header (plan §7.5 G16).
#[derive(Debug, Default, PartialEq, Eq)]
struct ContentType {
    mime_type: Option<String>,
    charset: Option<String>,
    boundary: Option<String>,
}

fn parse_content_type(header: &str) -> ContentType {
    let mut out = ContentType::default();
    let mut parts = header.split(';');
    if let Some(first) = parts.next() {
        let mime = first.trim();
        if !mime.is_empty() {
            out.mime_type = Some(mime.to_ascii_lowercase());
        }
    }
    for part in parts {
        let (k, v) = match part.split_once('=') {
            Some(kv) => (kv.0.trim().to_ascii_lowercase(), kv.1.trim()),
            None => continue,
        };
        let v = v.trim_matches('"').to_string();
        match k.as_str() {
            "charset" => out.charset = Some(v),
            "boundary" => out.boundary = Some(v),
            _ => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------
// crypto helpers
// ---------------------------------------------------------------------------

/// Hex digest of `data` with `algo` ∈ md5 | sha1 | sha256 | sha512.
fn hash_hex(algo: &str, data: &[u8]) -> Option<String> {
    use sha2::Digest;
    let out = match algo {
        "md5" => hex::encode(md5::Md5::digest(data)),
        "sha1" => hex::encode(sha1::Sha1::digest(data)),
        "sha256" => hex::encode(sha2::Sha256::digest(data)),
        "sha512" => hex::encode(sha2::Sha512::digest(data)),
        _ => return None,
    };
    Some(out)
}

/// Hex HMAC of `data` keyed with `secret`.
fn hmac_hex(algo: &str, secret: &[u8], data: &[u8]) -> Option<String> {
    use hmac::{Hmac, Mac};
    macro_rules! do_hmac {
        ($ty:ty) => {{
            let mut mac = Hmac::<$ty>::new_from_slice(secret).ok()?;
            mac.update(data);
            hex::encode(mac.finalize().into_bytes())
        }};
    }
    let out = match algo {
        "md5" => do_hmac!(md5::Md5),
        "sha1" => do_hmac!(sha1::Sha1),
        "sha256" => do_hmac!(sha2::Sha256),
        "sha512" => do_hmac!(sha2::Sha512),
        _ => return None,
    };
    Some(out)
}

// ---------------------------------------------------------------------------
// ESM import preprocessing
// ---------------------------------------------------------------------------

/// Result of scanning a script for `import … from "…"` statements.
struct Imports {
    /// Module sources to evaluate before the script, in import order.
    modules: Vec<(PathBuf, String)>,
    /// The script with import lines removed.
    script: String,
    /// Unresolvable imports (recorded, not fatal — JetBrains warns too).
    diagnostics: Vec<Diagnostic>,
}

/// Extract single-line `import` statements, resolve each specifier against
/// `base_dir`, read the module and strip its `export` keywords so the
/// declarations land in the global scope of the ES5.1 sandbox.
fn collect_imports(script: &str, base_dir: Option<&Path>) -> Imports {
    let mut imports = Imports {
        modules: Vec::new(),
        script: String::new(),
        diagnostics: Vec::new(),
    };
    let mut keep = String::with_capacity(script.len());
    for line in script.lines() {
        match import_specifier(line) {
            Some(spec) => {
                match resolve_module(spec, base_dir) {
                    Some((path, source)) => {
                        if !imports.modules.iter().any(|(p, _)| *p == path) {
                            imports.modules.push((path, source));
                        }
                    }
                    None => imports.diagnostics.push(diag(format!(
                        "cannot resolve import `{spec}` (base dir: {})",
                        base_dir
                            .map(|b| b.display().to_string())
                            .unwrap_or_else(|| ".".into())
                    ))),
                }
            }
            None => {
                keep.push_str(line);
                keep.push('\n');
            }
        }
    }
    imports.script = keep;
    imports
}

/// Recognise `import … from "spec"` / `import "spec"` on a single line.
fn import_specifier(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("import")?;
    // Must be followed by whitespace or a quote (`importx` is not an import).
    if !rest.starts_with(char::is_whitespace) && !rest.starts_with(['"', '\'']) {
        return None;
    }
    let from = match rest.find(" from ") {
        Some(i) => &rest[i + " from ".len()..],
        None => rest,
    };
    let from = from.trim().trim_end_matches(';').trim();
    let (quote, tail) = match from.chars().next() {
        Some(c @ ('"' | '\'')) => (c, &from[1..]),
        _ => return None,
    };
    let end = tail.find(quote)?;
    Some(&tail[..end])
}

/// Resolve a module specifier to `(path, source)`, trying the usual
/// extensions. Only relative/absolute file specifiers are supported.
fn resolve_module(spec: &str, base_dir: Option<&Path>) -> Option<(PathBuf, String)> {
    let base = PathBuf::from(spec);
    let candidates: Vec<PathBuf> = if base.is_absolute() {
        vec![base.clone()]
    } else {
        let dir = base_dir.unwrap_or(Path::new("."));
        vec![dir.join(&base)]
    };
    let mut tries = Vec::new();
    for c in candidates {
        tries.push(c.clone());
        for ext in [".js", ".mjs", ".cjs"] {
            let mut with_ext = c.clone().into_os_string();
            with_ext.push(ext);
            tries.push(PathBuf::from(with_ext));
        }
        for ext in [".js", ".mjs", ".cjs"] {
            tries.push(c.join(format!("index{ext}")));
        }
    }
    for path in tries {
        if path.is_file() {
            if let Ok(source) = std::fs::read_to_string(&path) {
                return Some((path, strip_exports(&source)));
            }
        }
    }
    None
}

/// Remove `export` / `export default` keywords so a module's declarations
/// become globals in the ES5.1 sandbox. `export { … }` lists are dropped.
fn strip_exports(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("export {") || trimmed.starts_with("export{") {
            continue;
        }
        if trimmed.starts_with("export default ") {
            out.push_str(&line.replacen("export default ", "", 1));
            out.push('\n');
            continue;
        }
        if trimmed.starts_with("export ") {
            out.push_str(&line.replacen("export ", "", 1));
            out.push('\n');
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// JS glue prelude
// ---------------------------------------------------------------------------

/// Builds the ergonomic API on top of `__native`. Evaluated at global scope so
/// `var`/`function` declarations are visible to the user script.
const GLUE_JS: &str = r#"
var __inTest = false;

function __parseJsonOrUndefined(s) {
    return (s === undefined || s === null) ? undefined : JSON.parse(s);
}

client.global = {
    set: function (k, v) {
        __native.globalSet(k, typeof v === "string" ? v : JSON.stringify(v));
    },
    get: function (k) { return __native.globalGet(k); },
    clear: function (k) { __native.globalClear(k); },
    clearAll: function () { __native.globalClearAll(); },
    all: function () { return JSON.parse(__native.globalAll()); }
};

client.test = function (name, fn) {
    var label = String(name);
    var ok = true;
    var msg = "";
    __inTest = true;
    try {
        fn();
    } catch (e) {
        ok = false;
        msg = (e && e.message !== undefined && e.message !== null) ? String(e.message) : String(e);
    }
    __inTest = false;
    __native.endTest(label, ok, msg);
};

client.assert = function (cond, msg) {
    if (cond) { return; }
    var m = (msg === undefined || msg === null) ? "Assertion failed" : String(msg);
    if (__inTest) { throw new Error(m); }
    __native.assertFailed(m);
};

client.exit = function () {
    __native.markExit();
    throw "__http_client_exit__";
};

var request = {
    method: __requestInfo.method,
    url: __requestInfo.url,
    headers: __requestInfo.headers,
    body: { tryGetSubstituted: function () { return __native.requestBody(); } },
    iteration: function () { return __native.iteration(); },
    templateValue: function (i) { return __parseJsonOrUndefined(__native.templateValue(i)); },
    variables: {
        set: function (k, v) { __native.varSet(k, JSON.stringify(v)); },
        get: function (k) { return __parseJsonOrUndefined(__native.varGet(k)); }
    },
    environment: {
        get: function (k) { return __parseJsonOrUndefined(__native.envGet(k)); }
    }
};

function __digestOf(hexValue) {
    return {
        toHex: function () { return hexValue; },
        toBase64: function () { return __native.hexToBase64(hexValue); }
    };
}

function __hasher(algo) {
    var text = "";
    var api = {
        updateWithText: function (t) { text = text + t; return api; },
        updateWithBase64: function (b) { text = text + __native.base64ToText(b); return api; },
        digest: function () { return __digestOf(__native.hash(algo, text)); }
    };
    return api;
}

function __hmacHasher(algo) {
    var secret = "";
    var text = "";
    var api = {
        withTextSecret: function (s) { secret = s; return api; },
        updateWithText: function (t) { text = text + t; return api; },
        digest: function () { return __digestOf(__native.hmacHash(algo, secret, text)); }
    };
    return api;
}

var crypto = {
    md5: function () { return __hasher("md5"); },
    sha1: function () { return __hasher("sha1"); },
    sha256: function () { return __hasher("sha256"); },
    sha512: function () { return __hasher("sha512"); },
    hmac: {
        md5: function () { return __hmacHasher("md5"); },
        sha1: function () { return __hmacHasher("sha1"); },
        sha256: function () { return __hmacHasher("sha256"); },
        sha512: function () { return __hmacHasher("sha512"); }
    }
};

function jsonPath(obj, expr) {
    return __parseJsonOrUndefined(__native.jsonPath(JSON.stringify(obj), expr));
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_type_parsing() {
        assert_eq!(
            parse_content_type("application/json"),
            ContentType {
                mime_type: Some("application/json".into()),
                charset: None,
                boundary: None
            }
        );
        assert_eq!(
            parse_content_type("text/plain; charset=UTF-8"),
            ContentType {
                mime_type: Some("text/plain".into()),
                charset: Some("UTF-8".into()),
                boundary: None
            }
        );
        assert_eq!(
            parse_content_type("multipart/form-data; boundary=abc"),
            ContentType {
                mime_type: Some("multipart/form-data".into()),
                charset: None,
                boundary: Some("abc".into())
            }
        );
        assert_eq!(parse_content_type(""), ContentType::default());
    }

    #[test]
    fn hash_and_hmac_vectors() {
        // RFC 6234 / standard test vectors.
        assert_eq!(
            hash_hex("sha256", b"abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hash_hex("md5", b"abc").unwrap(),
            "900150983cd24fb0d6963f7d28e17f72"
        );
        assert_eq!(
            hash_hex("sha1", b"abc").unwrap(),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            hash_hex("sha512", b"abc").unwrap(),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
             2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
        assert_eq!(hash_hex("nope", b"abc"), None);
        // RFC 4231 test case 2.
        assert_eq!(
            hmac_hex("sha256", b"Jefe", b"what do ya want for nothing?").unwrap(),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        assert_eq!(
            hmac_hex("md5", b"Jefe", b"what do ya want for nothing?").unwrap(),
            "750c783e6ab0b503eaa86e310a5db738"
        );
    }

    #[test]
    fn import_specifier_recognition() {
        assert_eq!(
            import_specifier(r#"import {makeSignature} from "./my-utils";"#),
            Some("./my-utils")
        );
        assert_eq!(
            import_specifier(r#"    import { a, b } from './mod.js'"#),
            Some("./mod.js")
        );
        assert_eq!(import_specifier(r#"import "./side-effect";"#), Some("./side-effect"));
        assert_eq!(import_specifier("import defaultName from \"m\";"), Some("m"));
        assert_eq!(import_specifier("client.global.set(\"import\", 1)"), None);
        assert_eq!(import_specifier("importantStuff();"), None);
    }

    #[test]
    fn strip_exports_keeps_declarations() {
        let src = "export function f() { return 1; }\nexport const K = 2;\nexport default 3;\nexport { f };\n";
        let out = strip_exports(src);
        assert!(out.contains("function f() { return 1; }"));
        assert!(out.contains("const K = 2;"));
        assert!(out.contains("3;"));
        assert!(!out.contains("export"));
    }

    #[test]
    fn resolve_module_finds_js_sibling() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("my-utils.js"), "export function f() {}\n").unwrap();
        let (path, source) = resolve_module("./my-utils", Some(tmp.path())).unwrap();
        assert_eq!(path.file_name().unwrap(), "my-utils.js");
        assert!(source.contains("function f()"));
        assert!(resolve_module("./missing", Some(tmp.path())).is_none());
    }
}
