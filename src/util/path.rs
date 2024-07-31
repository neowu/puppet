use std::path::Path;

use anyhow::Context;
use anyhow::Result;

pub trait PathExt {
    fn file_extension(&self) -> Result<&str>;
}

impl PathExt for Path {
    fn file_extension(&self) -> Result<&str> {
        let extension = self
            .extension()
            .with_context(|| format!("file must have extension, path={}", self.to_string_lossy()))?
            .to_str()
            .with_context(|| format!("path is invalid, path={}", self.to_string_lossy()))?;
        Ok(extension)
    }
}
