use std::fmt;

use crate::util::exception::Exception;
use serde::de;
use serde::Serialize;

pub fn from_json<'a, T>(json: &'a str) -> Result<T, Exception>
where
    T: de::Deserialize<'a>,
{
    let result = serde_json::from_str(json);
    result.map_err(|err| Exception::new(&format!("failed to deserialize json, error={err}, json={json}")))
}

pub fn to_json<T>(object: &T) -> Result<String, Exception>
where
    T: Serialize + fmt::Debug,
{
    let result = serde_json::to_string(object);
    result.map_err(|err| Exception::new(&format!("failed to serialize json, error={err}, object={object:?}")))
}
