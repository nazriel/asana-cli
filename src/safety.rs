use std::io::{IsTerminal, Write};

use serde::Serialize;
use serde_json::Value;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize)]
pub struct MutationPreview<'a> {
    pub dry_run: bool,
    pub method: &'a str,
    pub path: &'a str,
    pub query: &'a [(String, String)],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<&'a Value>,
    pub risk: &'a str,
    pub confirmation_required: bool,
}

pub fn require_destructive_confirmation(yes: bool, description: &str) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        return Err(Error::Confirmation(format!(
            "{description}; pass --yes to confirm non-interactively"
        )));
    }

    eprint!("{description}. Continue? [y/N] ");
    std::io::stderr().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    if matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        Ok(())
    } else {
        Err(Error::Confirmation("operation cancelled".to_owned()))
    }
}
