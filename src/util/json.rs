use std::fmt;

use anyhow::Context;
use anyhow::Result;
use serde::de;
use serde::Serialize;

pub fn from_json<'a, T>(json: &'a str) -> Result<T>
where
    T: de::Deserialize<'a>,
{
    serde_json::from_str(json).with_context(|| format!("json={json}"))
}

pub fn to_json<T>(object: &T) -> Result<String>
where
    T: Serialize + fmt::Debug,
{
    serde_json::to_string(object).with_context(|| format!("object={object:?}"))
}

#[allow(dead_code)]
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
