mod app;
mod auth;
mod cli;
mod client;
mod config;
mod download;
mod error;
mod models;
mod output;

use clap::{Parser, error::ErrorKind};

use crate::cli::Cli;

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                let _ = error.print();
                std::process::exit(0);
            }

            if std::env::args().any(|argument| argument == "--json") {
                eprintln!(
                    "{}",
                    serde_json::json!({
                        "error": {
                            "code": "usage",
                            "message": error.to_string(),
                            "status": null,
                            "request_id": null,
                        }
                    })
                );
            } else {
                let _ = error.print();
            }
            std::process::exit(2);
        }
    };
    let json = cli.json;

    if let Err(error) = app::run(cli) {
        output::print_error(&error, json);
        std::process::exit(error.exit_code());
    }
}
