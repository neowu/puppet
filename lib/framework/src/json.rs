use std::fmt;
use std::fs::read_to_string;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use serde::de;
use serde::Serialize;

pub fn load_file<T>(path: &Path) -> Result<T>
where
    T: de::DeserializeOwned,
{
    let json = read_to_string(path).with_context(|| format!("failed to read file, path={}", path.to_string_lossy()))?;
    serde_json::from_str(&json).with_context(|| format!("failed to deserialize, json={json}"))
}

pub fn from_json<'a, T>(json: &'a str) -> Result<T>
where
    T: de::Deserialize<'a>,
{
    serde_json::from_str(json).with_context(|| format!("failed to deserialize, json={json}"))
}

pub fn to_json<T>(object: &T) -> Result<String>
where
    T: Serialize + fmt::Debug,
{
    serde_json::to_string(object).with_context(|| format!("failed to serialize, object={object:?}"))
}

pub fn to_json_value<T>(enum_value: &T) -> Result<String>
where
    T: Serialize + fmt::Debug,
{
    let value = serde_json::to_string(enum_value).with_context(|| format!("enum={enum_value:?}"))?;
    Ok(value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(|value| value.to_string())
        .unwrap_or(value))
}
