//! Batch runner (plan §7.8 G23).
//!
//! Executes every [`Request`] in a parsed file in order, wiring together the
//! pieces landed in earlier phases:
//!
//! 1. pre-request script (`< {% … %}` / `< file.js`) → variable writes applied
//! 2. JSONPath loop expansion ([`expand_iterations`])
//! 3. per-iteration dispatch ([`Dispatcher::send_prepared_with`])
//! 4. response handler (`> {% … %}` / `> file.js`) → tests / diagnostics
//! 5. `>>` output redirect and `<>` response ref persistence
//!
//! A single [`HandlerRuntime`] is reused for the whole file so `client.global`
//! state persists across requests, mirroring the JetBrains HTTP Client session.
//! `client.exit()` stops the run early.

use crate::dispatch::{
    write_output_redirect, write_response_ref, DispatchResponse, Dispatcher, SendOptions,
};
use crate::env::{Environment, Layer, VarValue};
use crate::error::{CoreError, Diagnostic, ErrorKind, Result, Span};
use crate::execute::{expand_iterations, prepare_with, DiskResolver};
use crate::handler::{HandlerInput, HandlerRuntime, RequestInfo, TestResult};
use crate::model::{Request, RequestsFile, ResponseHandler};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Options for a batch run. The base [`Environment`] (env-file values, project
/// root) is supplied separately so callers control env loading.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Base directory for `< file` / `> file` references and relative output
    /// paths. Defaults to the `.http` file's parent directory.
    pub base_dir: Option<PathBuf>,
    /// Run only the request with this `### name` (or 1-based position).
    /// `None` runs every request in the file.
    pub request: Option<String>,
}

/// Outcome of a single loop iteration (one dispatch + handler run).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IterationReport {
    pub index: usize,
    pub url: String,
    pub status: u16,
    pub elapsed_ms: u64,
    pub tests: Vec<TestResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<String>,
}

/// Outcome of one request (all its iterations).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestReport {
    pub name: Option<String>,
    pub iterations: Vec<IterationReport>,
}

/// Aggregate outcome of a whole file run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileReport {
    pub requests: Vec<RequestReport>,
    pub tests_passed: usize,
    pub tests_failed: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<String>,
}

impl FileReport {
    /// `true` when every `client.test` assertion passed (and none failed).
    pub fn ok(&self) -> bool {
        self.tests_failed == 0
    }
}

/// Read, parse and run a `.http` file with a default [`Dispatcher`].
pub async fn run_file(path: &Path, env: Environment, opts: &RunOptions) -> Result<FileReport> {
    run_file_with(path, env, opts, &Dispatcher::new()).await
}

/// Like [`run_file`] but with a caller-supplied dispatcher (e.g. one wrapping a
/// mock-server client for offline tests).
pub async fn run_file_with(
    path: &Path,
    env: Environment,
    opts: &RunOptions,
    dispatcher: &Dispatcher,
) -> Result<FileReport> {
    let src = std::fs::read_to_string(path).map_err(|e| {
        CoreError::new(
            ErrorKind::Io,
            Span::new(0, 0),
            format!("failed to read `{}`: {e}", path.display()),
        )
    })?;
    let file = crate::parser::parse_file(&src)?;
    let base_dir = opts
        .base_dir
        .clone()
        .or_else(|| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    run_parsed(
        &file,
        env,
        &base_dir,
        opts.request.as_deref(),
        dispatcher,
    )
    .await
}

/// Run an already-parsed file. `base_env` carries env-file values; the runner
/// clones it per request and overlays script globals + per-iteration bindings.
pub async fn run_parsed(
    file: &RequestsFile,
    base_env: Environment,
    base_dir: &Path,
    request_filter: Option<&str>,
    dispatcher: &Dispatcher,
) -> Result<FileReport> {
    let mut runtime = HandlerRuntime::new();
    let resolver = DiskResolver::new(base_dir);
    let mut report = FileReport::default();

    for req in select_requests(&file.requests, request_filter) {
        // Fresh working env per request: env-file layers + script globals set
        // by earlier requests. The Request layer starts empty.
        let mut work = base_env.clone();
        work.set_project_root(base_dir);
        overlay_globals(&runtime, &mut work);

        // Initial prepare so the pre-request script sees a populated `request`.
        let prepared0 = prepare_with(req, &work, &resolver)?;
        let mut info = request_info(&prepared0, 0, Vec::new());

        if let Some(prs) = &req.pre_request_script {
            run_handler(&mut runtime, prs, None, &info, &work, base_dir)?;
            apply_writes(&mut runtime, &mut work);
            overlay_globals(&runtime, &mut work);
        }

        let iterations = expand_iterations(req, &work);
        let mut req_report = RequestReport {
            name: req.name.clone(),
            iterations: Vec::new(),
        };

        for it in iterations {
            // Bind this iteration's JSONPath template values into the Request
            // layer so `{{$.clients..id}}` substitutes to the bound value.
            for (key, value) in &it.bindings {
                work.set_value(Layer::Request, key.clone(), value.clone());
            }
            let prepared = prepare_with(req, &work, &resolver)?;
            info = request_info(&prepared, it.index, it.template_values.clone());

            let opts = SendOptions::from_request(req);
            let resp = dispatcher.send_prepared_with(&prepared, &opts).await?;

            if let Some(rh) = &req.response_handler {
                run_handler(&mut runtime, rh, Some(&resp), &info, &work, base_dir)?;
            }

            let tests = runtime.take_tests();
            let diagnostics = runtime.take_diagnostics();
            let logs = runtime.take_logs();
            apply_writes(&mut runtime, &mut work);

            if let Some(redirect) = &req.output_redirect {
                write_output_redirect(&resp, redirect, &work, Some(base_dir))?;
            }
            if let Some(response_ref) = &req.response_ref {
                write_response_ref(&resp, response_ref, Some(base_dir))?;
            }

            for t in &tests {
                if t.passed {
                    report.tests_passed += 1;
                } else {
                    report.tests_failed += 1;
                }
            }
            report.logs.extend(logs.iter().cloned());
            req_report.iterations.push(IterationReport {
                index: it.index,
                url: resp.url.clone(),
                status: resp.status,
                elapsed_ms: resp.elapsed.as_millis() as u64,
                tests,
                diagnostics,
                logs,
            });

            if runtime.exit_requested() {
                break;
            }
        }

        report.requests.push(req_report);
        if runtime.exit_requested() {
            break;
        }
    }

    Ok(report)
}

/// Pick the requests to run: all of them, or the one matching `filter` by name
/// or 1-based position. An unmatched filter yields an empty selection.
fn select_requests<'a>(all: &'a [Request], filter: Option<&str>) -> Vec<&'a Request> {
    match filter {
        None => all.iter().collect(),
        Some(s) => {
            if let Some(r) = all.iter().find(|r| r.name.as_deref() == Some(s)) {
                return vec![r];
            }
            if let Ok(idx) = s.parse::<usize>() {
                if idx >= 1 && idx <= all.len() {
                    return vec![&all[idx - 1]];
                }
            }
            Vec::new()
        }
    }
}

/// Mirror the handler runtime's `client.global.*` store into `env`'s Global
/// layer so later requests can substitute `{{key}}` references to them.
fn overlay_globals(runtime: &HandlerRuntime, env: &mut Environment) {
    for (k, v) in runtime.globals() {
        env.set_value(Layer::Global, k, VarValue::Str(v));
    }
}

/// Apply the variable writes a script requested (`request.variables.set`).
fn apply_writes(runtime: &mut HandlerRuntime, env: &mut Environment) {
    for w in runtime.take_var_writes() {
        env.set_value(w.layer, w.key, w.value);
    }
}

/// Build the `request` object exposed to scripts from a prepared request.
fn request_info(
    prepared: &crate::execute::PreparedRequest,
    iteration: usize,
    template_values: Vec<serde_json::Value>,
) -> RequestInfo {
    RequestInfo {
        method: prepared.method.as_str().to_string(),
        url: prepared.url.clone(),
        headers: prepared
            .headers
            .iter()
            .map(|h| (h.name.clone(), h.value.clone()))
            .collect(),
        body: prepared
            .body
            .as_ref()
            .map(|b| String::from_utf8_lossy(b).into_owned()),
        iteration,
        template_values,
    }
}

/// Run an inline or file-referenced handler script. `response` is `None` for
/// pre-request scripts. File refs resolve against `base_dir`; imports inside a
/// file handler resolve against the script's own directory.
fn run_handler(
    runtime: &mut HandlerRuntime,
    handler: &ResponseHandler,
    response: Option<&DispatchResponse>,
    info: &RequestInfo,
    env: &Environment,
    base_dir: &Path,
) -> Result<()> {
    match handler {
        ResponseHandler::Inline { script } => runtime.run_with(
            script,
            HandlerInput {
                response,
                request: info,
                env,
                base_dir: Some(base_dir),
            },
        ),
        ResponseHandler::FileRef { path } => {
            let resolved = resolve_path(base_dir, path);
            let script = std::fs::read_to_string(&resolved).map_err(|e| {
                CoreError::new(
                    ErrorKind::Io,
                    Span::new(0, 0),
                    format!("failed to read handler `{}`: {e}", resolved.display()),
                )
            })?;
            let script_dir = resolved
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| base_dir.to_path_buf());
            runtime.run_with(
                &script,
                HandlerInput {
                    response,
                    request: info,
                    env,
                    base_dir: Some(&script_dir),
                },
            )
        }
    }
}

/// Resolve a possibly-relative path against `base_dir`.
fn resolve_path(base_dir: &Path, path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        p
    } else {
        base_dir.join(p)
    }
}
