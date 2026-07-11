use std::{fs, io::Read};

use serde_json::Value;

use crate::error::{Error, Result};

pub fn read_json(source: &str) -> Result<Value> {
    let contents = if source == "-" {
        let mut contents = String::new();
        std::io::stdin().read_to_string(&mut contents)?;
        contents
    } else if let Some(path) = source.strip_prefix('@') {
        fs::read_to_string(path)?
    } else {
        source.to_owned()
    };

    let value: Value = serde_json::from_str(&contents)
        .map_err(|error| Error::Usage(format!("invalid JSON input: {error}")))?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(Error::Usage("JSON input must be an object".to_owned()))
    }
}

pub fn parse_key_value(value: &str) -> std::result::Result<(String, String), String> {
    let (key, value) = value
        .split_once('=')
        .ok_or_else(|| "expected KEY=VALUE".to_owned())?;
    if key.is_empty() {
        return Err("key cannot be empty".to_owned());
    }
    Ok((key.to_owned(), value.to_owned()))
}

pub fn read_text(source: &str) -> Result<String> {
    if source == "-" {
        let mut contents = String::new();
        std::io::stdin().read_to_string(&mut contents)?;
        Ok(contents)
    } else {
        Ok(fs::read_to_string(source)?)
    }
}

pub fn merge_objects(base: &mut Value, extra: Value) -> Result<()> {
    let base = base
        .as_object_mut()
        .ok_or_else(|| Error::Usage("internal mutation data must be an object".to_owned()))?;
    let extra = extra
        .as_object()
        .ok_or_else(|| Error::Usage("JSON input must be an object".to_owned()))?;
    for (key, value) in extra {
        if base.contains_key(key) {
            return Err(Error::Usage(format!(
                "field {key:?} was supplied both as a typed flag and through --data"
            )));
        }
        base.insert(key.clone(), value.clone());
    }
    Ok(())
}

pub fn parse_json_assignment(value: &str) -> std::result::Result<(String, Value), String> {
    let (key, raw) = value
        .split_once('=')
        .ok_or_else(|| "expected GID=JSON_VALUE".to_owned())?;
    if key.is_empty() {
        return Err("GID cannot be empty".to_owned());
    }
    let value = serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_owned()));
    Ok((key.to_owned(), value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_inline_json_object() {
        let value = read_json(r#"{"name":"Example"}"#).expect("valid object");
        assert_eq!(value["name"], "Example");
    }

    #[test]
    fn rejects_non_object_json() {
        let error = read_json("[]").expect_err("array must be rejected");
        assert_eq!(error.code(), "usage");
    }

    #[test]
    fn parses_key_value_at_first_equals() {
        assert_eq!(
            parse_key_value("query=a=b").expect("valid pair"),
            ("query".to_owned(), "a=b".to_owned())
        );
    }
}
