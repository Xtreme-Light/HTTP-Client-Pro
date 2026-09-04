//! `http-cli` — thin command-line wrapper around `http-core`.
//!
//! Subcommands:
//! - `run <file.http>` — execute the first (or named) request in a `.http`
//!   file and print the captured response.
//!
//! Flags (Phase 5 P5-2):
//! - `--request <name>` — execute the request whose `### name` separator
//!   matches. Falls back to position (1-based) if `name` is numeric.
//! - `--env <name>` — select a named environment from
//!   `http-client.env.json` in the working directory (JetBrains format).
//! - `--env-file <path>` — override the env file location.
//! - `--json` — print the result as JSON (default is human-readable).
//! - `--cwd <path>` — base directory for `< file` references and response
//!   refs (`<>`); defaults to the .http file's parent dir.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use http_core::dispatch::Dispatcher;
use http_core::env::Environment;
use http_core::parser::parse_file;
use serde::Deserialize;

/// Errors surfaced to the user via the CLI's exit code / stderr.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("usage: http-client-pro run <file.http> [options]")]
    Usage,
    #[error("read {0}: {1}")]
    Read(PathBuf, String),
    #[error("parse: {0}")]
    Parse(String),
    #[error("request not found: {0}")]
    RequestNotFound(String),
    #[error("env file {0}: {1}")]
    EnvFile(PathBuf, String),
    #[error("environment {0:?} not found in {1}")]
    EnvNotFound(String, PathBuf),
    #[error("dispatch: {0}")]
    Dispatch(String),
}

impl CliError {
    /// Map to a process exit code. Usage → 2, anything else → 1.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage => 2,
            _ => 1,
        }
    }
}

/// Parsed CLI invocation. Built by [`parse_args`]; use [`Cli::run`] to
/// execute and consume the owned values.
#[derive(Debug, Clone)]
pub struct Cli {
    pub subcommand: Subcommand,
}

#[derive(Debug, Clone)]
pub enum Subcommand {
    Run(RunArgs),
}

#[derive(Debug, Clone, Default)]
pub struct RunArgs {
    pub file: PathBuf,
    pub request: Option<String>,
    pub env: Option<String>,
    pub env_file: Option<PathBuf>,
    pub json: bool,
    pub cwd: Option<PathBuf>,
}

/// Parse raw `std::env::args`-style argument vector into a [`Cli`] struct.
pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Cli, CliError> {
    let mut iter = args.into_iter();
    // Skip argv[0] (program name).
    let _program = iter.next();
    let sub = iter.next().ok_or(CliError::Usage)?;
    match sub.as_str() {
        "run" => {
            let mut args = RunArgs::default();
            let mut positional: Vec<String> = Vec::new();
            while let Some(tok) = iter.next() {
                match tok.as_str() {
                    "--request" | "-r" => {
                        args.request = Some(iter.next().ok_or(CliError::Usage)?);
                    }
                    "--env" | "-e" => {
                        args.env = Some(iter.next().ok_or(CliError::Usage)?);
                    }
                    "--env-file" => {
                        args.env_file = Some(PathBuf::from(iter.next().ok_or(CliError::Usage)?));
                    }
                    "--json" | "-j" => {
                        args.json = true;
                    }
                    "--cwd" => {
                        args.cwd = Some(PathBuf::from(iter.next().ok_or(CliError::Usage)?));
                    }
                    "--help" | "-h" => {
                        return Err(CliError::Usage);
                    }
                    _ if tok.starts_with("--") => {
                        // Unknown long flag — accept silently for forward-compat.
                    }
                    _ => positional.push(tok),
                }
            }
            args.file = positional
                .into_iter()
                .next()
                .map(PathBuf::from)
                .ok_or(CliError::Usage)?;
            Ok(Cli {
                subcommand: Subcommand::Run(args),
            })
        }
        "--help" | "-h" | "help" => Err(CliError::Usage),
        other => {
            let _ = other;
            Err(CliError::Usage)
        }
    }
}

impl Cli {
    /// Execute the parsed invocation. Returns the process exit code.
    pub async fn run(self) -> ExitCode {
        match self.subcommand {
            Subcommand::Run(args) => match run_request(args).await {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(e.exit_code())
                }
            },
        }
    }
}

async fn run_request(args: RunArgs) -> Result<(), CliError> {
    let src = std::fs::read_to_string(&args.file)
        .map_err(|e| CliError::Read(args.file.clone(), e.to_string()))?;
    let file = parse_file(&src).map_err(|e| CliError::Parse(e.to_string()))?;
    if !file.diagnostics.is_empty() {
        for d in &file.diagnostics {
            eprintln!("warning: {} at {}", d.message, d.span);
        }
    }
    let req = select_request(&file.requests, args.request.as_deref())?;
    let cwd = args
        .cwd
        .clone()
        .or_else(|| args.file.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let env = load_env(args.env.as_deref(), args.env_file.as_deref(), &cwd)?;
    let dispatcher = Dispatcher::new();
    let res = dispatcher
        .send(req, &env, Some(&cwd))
        .await
        .map_err(|e| CliError::Dispatch(e.to_string()))?;
    if args.json {
        print_json(&res);
    } else {
        print_human(&res);
    }
    Ok(())
}

/// Select the request to run. `selection`:
/// - `None` → first request.
/// - `Some(name)` — if a request has a matching `name`, use it; otherwise,
///   if `name` parses as a 1-based index, use position; otherwise error.
pub fn select_request<'a>(
    requests: &'a [http_core::model::Request],
    selection: Option<&str>,
) -> Result<&'a http_core::model::Request, CliError> {
    match selection {
        None => requests
            .first()
            .ok_or_else(|| CliError::RequestNotFound("(empty file)".to_string())),
        Some(s) => {
            if let Some(req) = requests.iter().find(|r| r.name.as_deref() == Some(s)) {
                return Ok(req);
            }
            if let Ok(idx) = s.parse::<usize>() {
                if idx >= 1 && idx <= requests.len() {
                    return Ok(&requests[idx - 1]);
                }
            }
            Err(CliError::RequestNotFound(s.to_string()))
        }
    }
}

/// JetBrains HTTP Client env file: `{ "env_name": { "var": "value" } }`.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct EnvFile {
    #[serde(flatten)]
    envs: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
}

pub fn load_env(
    env_name: Option<&str>,
    env_file: Option<&Path>,
    cwd: &Path,
) -> Result<Environment, CliError> {
    let Some(name) = env_name else {
        return Ok(Environment::new());
    };
    let path = env_file
        .map(Path::to_path_buf)
        .unwrap_or_else(|| cwd.join("http-client.env.json"));
    let text = std::fs::read_to_string(&path)
        .map_err(|e| CliError::EnvFile(path.clone(), e.to_string()))?;
    let parsed: EnvFile =
        serde_json::from_str(&text).map_err(|e| CliError::EnvFile(path.clone(), e.to_string()))?;
    let vars = parsed
        .envs
        .get(name)
        .ok_or_else(|| CliError::EnvNotFound(name.to_string(), path.clone()))?;
    let mut env = Environment::new();
    for (k, v) in vars {
        env.set(k, v);
    }
    Ok(env)
}

fn print_json(res: &http_core::dispatch::DispatchResponse) {
    let mut headers = serde_json::Map::new();
    for (n, v) in &res.headers {
        headers.insert(n.clone(), serde_json::Value::from(v.clone()));
    }
    let body = String::from_utf8_lossy(&res.body).into_owned();
    let out = serde_json::json!({
        "status": res.status,
        "headers": headers,
        "body": body,
        "elapsed_ms": res.elapsed.as_millis() as u64,
        "url": res.url,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}

fn print_human(res: &http_core::dispatch::DispatchResponse) {
    println!("{} {}", res.status, res.url);
    println!("elapsed: {} ms", res.elapsed.as_millis());
    println!();
    for (n, v) in &res.headers {
        println!("{n}: {v}");
    }
    println!();
    println!("{}", String::from_utf8_lossy(&res.body));
}
