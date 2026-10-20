use std::{
    env,
    process::{Command, Stdio},
    time::Duration,
};

use keyring::Entry;

use crate::{
    client::AsanaClient,
    error::{Error, Result},
    models::User,
};

const KEYRING_SERVICE: &str = "asana-cli";
const DEVELOPER_CONSOLE_URL: &str = "https://app.asana.com/0/my-apps";

#[derive(Debug, Clone, Copy)]
pub enum TokenSource {
    Environment,
    Keyring,
}

impl TokenSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Keyring => "keyring",
        }
    }
}

pub fn resolve_token(profile: &str) -> Result<(String, TokenSource)> {
    if let Ok(token) = env::var("ASANA_ACCESS_TOKEN")
        && !token.trim().is_empty()
    {
        return Ok((token, TokenSource::Environment));
    }

    let entry = keyring_entry(profile)?;
    let token = entry.get_password().map_err(|error| {
        Error::Auth(format!(
            "no token found in the keyring ({error}); run `asana auth login` or set ASANA_ACCESS_TOKEN"
        ))
    })?;

    if token.trim().is_empty() {
        return Err(Error::Auth(
            "the keyring contains an empty token; run `asana auth login`".to_owned(),
        ));
    }

    Ok((token, TokenSource::Keyring))
}

pub fn login(
    profile: &str,
    no_browser: bool,
    verbose: bool,
    request_timeout: Duration,
) -> Result<User> {
    eprintln!("Create a personal access token in the Asana developer console:");
    eprintln!();
    eprintln!("  {DEVELOPER_CONSOLE_URL}");
    eprintln!();
    if !no_browser && open_browser(DEVELOPER_CONSOLE_URL) {
        eprintln!(
            "Opened it in your browser. Choose \"Create new token\", copy the token, and paste it below."
        );
    } else {
        eprintln!(
            "Open it in a browser, choose \"Create new token\", copy the token, and paste it below."
        );
    }

    let token = rpassword::prompt_password("Asana personal access token: ")
        .map_err(|error| Error::Auth(format!("could not read token: {error}")))?;

    if token.trim().is_empty() {
        return Err(Error::Usage("token cannot be empty".to_owned()));
    }

    let client = AsanaClient::new_with_options(token.clone(), verbose, None, request_timeout)?;
    let user = client.current_user()?;

    keyring_entry(profile)?
        .set_password(&token)
        .map_err(|error| {
            Error::Auth(format!(
                "could not store token in the operating-system keyring: {error}"
            ))
        })?;

    Ok(user)
}

pub fn logout(profile: &str) -> Result<()> {
    keyring_entry(profile)?
        .delete_credential()
        .map_err(|error| {
            Error::Auth(format!(
                "could not remove the token from the keyring: {error}"
            ))
        })
}

/// Best-effort: returns whether a browser launcher was started. Failures fall
/// back to the printed URL, so they are not reported.
fn open_browser(url: &str) -> bool {
    let mut command = if cfg!(target_os = "macos") {
        Command::new("open")
    } else if cfg!(windows) {
        // `start` treats its first quoted argument as the window title.
        let mut command = Command::new("cmd");
        command.args(["/C", "start", ""]);
        command
    } else {
        // Without a display, xdg-open may start a text browser in this terminal.
        if env::var_os("DISPLAY").is_none() && env::var_os("WAYLAND_DISPLAY").is_none() {
            return false;
        }
        Command::new("xdg-open")
    };
    command
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn keyring_entry(profile: &str) -> Result<Entry> {
    Entry::new(KEYRING_SERVICE, profile).map_err(|error| {
        Error::Auth(format!(
            "could not access the operating-system keyring: {error}"
        ))
    })
}
