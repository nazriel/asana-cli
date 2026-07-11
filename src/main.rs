use clap::{FromArgMatches, error::ErrorKind};

use asana_cli::{app, cli::Cli, output};

fn main() {
    let cli = match Cli::command_sorted()
        .try_get_matches()
        .and_then(|matches| Cli::from_arg_matches(&matches))
    {
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
                            "ambiguous": false,
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
