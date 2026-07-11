use std::{fs, path::PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub workspace_gid: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)?;
        toml::from_str(&contents)
            .map_err(|error| Error::Config(format!("could not read {}: {error}", path.display())))
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        let parent = path.parent().ok_or_else(|| {
            Error::Config(format!(
                "configuration path has no parent: {}",
                path.display()
            ))
        })?;
        fs::create_dir_all(parent)?;

        let contents = toml::to_string_pretty(self).map_err(|error| {
            Error::Config(format!("could not serialize configuration: {error}"))
        })?;
        fs::write(path, contents)?;
        Ok(())
    }
}

pub fn config_path() -> Result<PathBuf> {
    let directories = ProjectDirs::from("com", "asana-cli", "asana-cli").ok_or_else(|| {
        Error::Config("could not determine the platform config directory".to_owned())
    })?;
    Ok(directories.config_dir().join("config.toml"))
}

pub fn require_workspace(override_gid: Option<&str>, config: &Config) -> Result<String> {
    override_gid
        .map(ToOwned::to_owned)
        .or_else(|| config.workspace_gid.clone())
        .ok_or_else(|| {
            Error::Config(
                "no workspace is selected; run `asana workspace use <gid>` or pass --workspace <gid>"
                    .to_owned(),
            )
        })
}
