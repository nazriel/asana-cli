use std::{collections::BTreeMap, fs, path::PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const DEFAULT_PROFILE: &str = "default";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profiles: BTreeMap<String, ProfileConfig>,
    // Backward-compatible 0.1 field. It is migrated to profiles.default on load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_gid: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_gid: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)?;
        let mut config: Self = toml::from_str(&contents).map_err(|error| {
            Error::Config(format!("could not read {}: {error}", path.display()))
        })?;
        config.migrate_legacy();
        Ok(config)
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

    pub fn selected_profile<'a>(&'a self, override_name: Option<&'a str>) -> &'a str {
        override_name
            .or(self.active_profile.as_deref())
            .unwrap_or(DEFAULT_PROFILE)
    }

    pub fn profile(&self, name: &str) -> ProfileConfig {
        self.profiles.get(name).cloned().unwrap_or_default()
    }

    pub fn profile_mut(&mut self, name: &str) -> &mut ProfileConfig {
        self.profiles.entry(name.to_owned()).or_default()
    }

    pub fn profile_names(&self) -> Vec<String> {
        let mut names = self.profiles.keys().cloned().collect::<Vec<_>>();
        if !names.iter().any(|name| name == DEFAULT_PROFILE) {
            names.push(DEFAULT_PROFILE.to_owned());
            names.sort();
        }
        names
    }

    fn migrate_legacy(&mut self) {
        if let Some(workspace_gid) = self.workspace_gid.take() {
            self.profiles
                .entry(DEFAULT_PROFILE.to_owned())
                .or_default()
                .workspace_gid
                .get_or_insert(workspace_gid);
        }
    }
}

pub fn config_path() -> Result<PathBuf> {
    let directories = ProjectDirs::from("com", "asana-cli", "asana-cli").ok_or_else(|| {
        Error::Config("could not determine the platform config directory".to_owned())
    })?;
    Ok(directories.config_dir().join("config.toml"))
}

pub fn require_workspace(
    override_gid: Option<&str>,
    config: &Config,
    profile: &str,
) -> Result<String> {
    override_gid
        .map(ToOwned::to_owned)
        .or_else(|| config.profile(profile).workspace_gid)
        .ok_or_else(|| {
            Error::Config(
                format!(
                    "no workspace is selected for profile {profile:?}; run `asana --profile {profile} workspace use <gid>` or pass --workspace <gid>"
                ),
            )
        })
}

pub fn validate_profile_name(name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    if valid {
        Ok(())
    } else {
        Err(Error::Usage(
            "profile names must contain only letters, digits, '-' or '_' and be at most 64 characters"
                .to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_legacy_workspace_to_default_profile() {
        let mut config: Config = toml::from_str("workspace_gid = \"123\"").expect("legacy config");
        config.migrate_legacy();
        assert_eq!(
            config.profile(DEFAULT_PROFILE).workspace_gid.as_deref(),
            Some("123")
        );
        assert!(config.workspace_gid.is_none());
    }

    #[test]
    fn validates_profile_names() {
        assert!(validate_profile_name("work-prod_2").is_ok());
        assert!(validate_profile_name("bad profile").is_err());
    }
}
