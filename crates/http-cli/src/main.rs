//! Thin binary entry point: parse args, hand off to `http_cli::Cli::run`.

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match http_cli::parse_args(args) {
        Ok(cli) => cli.run().await,
        Err(e) => {
            eprintln!("{e}");
            eprintln!();
            eprintln!(
                "usage: http-client-pro run <file.http> [--request <name>] [--env <name>] [--json]"
            );
            std::process::ExitCode::from(e.exit_code())
        }
    }
}
