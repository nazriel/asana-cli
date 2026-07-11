use serde::Deserialize;

use crate::error::{Error, Result};

#[derive(Debug, Deserialize)]
struct Manifest {
    resources: Vec<ResourceFamily>,
}

#[derive(Debug, Deserialize)]
struct ResourceFamily {
    prefixes: Vec<String>,
}

pub fn ensure_catalogued_path(path: &str, allow_unknown: bool) -> Result<()> {
    if allow_unknown {
        return Ok(());
    }
    let manifest: Manifest =
        toml::from_str(include_str!("../api-support.toml")).map_err(|error| {
            Error::Config(format!("embedded API support manifest is invalid: {error}"))
        })?;
    let first = path
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or_default();
    if manifest
        .resources
        .iter()
        .flat_map(|resource| &resource.prefixes)
        .any(|prefix| prefix == first)
    {
        Ok(())
    } else {
        Err(Error::Usage(format!(
            "API path {path:?} is outside the vendored Asana catalog; check the path or pass --allow-unknown-endpoint"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_catalogued_nested_path() {
        ensure_catalogued_path("/workspaces/123/tasks/search", false).expect("known path");
    }

    #[test]
    fn rejects_unknown_path_by_default() {
        assert!(ensure_catalogued_path("/future_resource/123", false).is_err());
    }

    #[test]
    fn allows_verified_new_path_with_escape_hatch() {
        ensure_catalogued_path("/future_resource/123", true).expect("explicit override");
    }
}
