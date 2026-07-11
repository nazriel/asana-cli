use std::{env, time::Duration};

use keyring::Entry;

use crate::{
    client::AsanaClient,
    error::{Error, Result},
    models::User,
};

const KEYRING_SERVICE: &str = "asana-cli";

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

pub fn login(profile: &str, verbose: bool, request_timeout: Duration) -> Result<User> {
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

fn keyring_entry(profile: &str) -> Result<Entry> {
    Entry::new(KEYRING_SERVICE, profile).map_err(|error| {
        Error::Auth(format!(
            "could not access the operating-system keyring: {error}"
        ))
    })
}
