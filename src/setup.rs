use std::{env, fs, path::PathBuf};

use clap_complete::{Shell, generate};
use directories::BaseDirs;
use serde::Serialize;

use crate::{
    cli::{Cli, CompletionShell},
    error::{Error, Result},
    output,
};

#[derive(Debug, Serialize)]
struct SetupResult {
    shell: &'static str,
    path: PathBuf,
    installed: bool,
    activation_hint: String,
}

pub fn run(requested_shell: Option<CompletionShell>, json: bool) -> Result<()> {
    let shell = requested_shell.map_or_else(detect_shell, Ok)?;
    let path = completion_path(shell)?;
    let parent = path.parent().ok_or_else(|| {
        Error::Config(format!("completion path has no parent: {}", path.display()))
    })?;
    fs::create_dir_all(parent)?;

    let mut command = Cli::command_sorted();
    let mut script = Vec::new();
    generate(clap_shell(shell), &mut command, "asana", &mut script);
    atomic_write(&path, &script)?;

    let result = SetupResult {
        shell: shell_name(shell),
        activation_hint: activation_hint(shell, &path),
        path,
        installed: true,
    };
    if json {
        output::emit_json(&result)
    } else {
        println!(
            "Installed {} completion at {}",
            result.shell,
            result.path.display()
        );
        println!("{}", result.activation_hint);
        Ok(())
    }
}

fn detect_shell() -> Result<CompletionShell> {
    let value = env::var_os("SHELL").ok_or_else(|| {
        Error::Usage(
            "could not detect the current shell from $SHELL; pass --shell bash, --shell zsh, or --shell fish"
                .to_owned(),
        )
    })?;
    let name = PathBuf::from(value)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match name.as_str() {
        "bash" => Ok(CompletionShell::Bash),
        "fish" => Ok(CompletionShell::Fish),
        "zsh" => Ok(CompletionShell::Zsh),
        _ => Err(Error::Usage(format!(
            "unsupported shell {name:?}; pass --shell bash, --shell zsh, or --shell fish"
        ))),
    }
}

fn completion_path(shell: CompletionShell) -> Result<PathBuf> {
    let base = BaseDirs::new()
        .ok_or_else(|| Error::Config("could not determine the user's home directory".to_owned()))?;
    match shell {
        CompletionShell::Bash => {
            Ok(data_home(base.home_dir()).join("bash-completion/completions/asana"))
        }
        CompletionShell::Fish => {
            Ok(config_home(base.home_dir()).join("fish/completions/asana.fish"))
        }
        CompletionShell::Zsh => Ok(data_home(base.home_dir()).join("zsh/site-functions/_asana")),
    }
}

fn data_home(home: &std::path::Path) -> PathBuf {
    absolute_env_path("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"))
}

fn config_home(home: &std::path::Path) -> PathBuf {
    absolute_env_path("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"))
}

fn absolute_env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

fn atomic_write(path: &std::path::Path, contents: &[u8]) -> Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("asana");
    let temporary = path.with_file_name(format!(".{file_name}.tmp.{}", std::process::id()));
    fs::write(&temporary, contents)?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(())
}

fn clap_shell(shell: CompletionShell) -> Shell {
    match shell {
        CompletionShell::Bash => Shell::Bash,
        CompletionShell::Fish => Shell::Fish,
        CompletionShell::Zsh => Shell::Zsh,
    }
}

fn shell_name(shell: CompletionShell) -> &'static str {
    match shell {
        CompletionShell::Bash => "bash",
        CompletionShell::Fish => "fish",
        CompletionShell::Zsh => "zsh",
    }
}

fn activation_hint(shell: CompletionShell, path: &std::path::Path) -> String {
    match shell {
        CompletionShell::Bash => {
            format!("Open a new Bash session, or run: source {}", path.display())
        }
        CompletionShell::Fish => {
            format!("Open a new Fish session, or run: source {}", path.display())
        }
        CompletionShell::Zsh => format!(
            "Ensure {} is in fpath, then restart Zsh or run: autoload -Uz compinit && compinit",
            path.parent().unwrap_or(path).display()
        ),
    }
}
