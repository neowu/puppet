use std::fmt;

use serde::de;
use serde::Serialize;

use crate::util::exception::Exception;

pub fn from_json<'a, T>(json: &'a str) -> Result<T, Exception>
where
    T: de::Deserialize<'a>,
{
    serde_json::from_str(json).map_err(|err| Exception::unexpected_with_context(err, &format!("json={json}")))
}

pub fn to_json<T>(object: &T) -> Result<String, Exception>
where
    T: Serialize + fmt::Debug,
{
    serde_json::to_string(object).map_err(|err| Exception::unexpected_with_context(err, &format!("object={object:?}")))
}
