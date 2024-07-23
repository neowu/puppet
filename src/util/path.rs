use std::path::Path;

use super::exception::Exception;

pub trait PathExt {
    fn file_extension(&self) -> Result<&str, Exception>;
}

impl PathExt for Path {
    fn file_extension(&self) -> Result<&str, Exception> {
        let extension = self
            .extension()
            .ok_or_else(|| Exception::ValidationError(format!("file must have extension, path={}", self.to_string_lossy())))?
            .to_str()
            .ok_or_else(|| Exception::ValidationError(format!("path is invalid, path={}", self.to_string_lossy())))?;
        Ok(extension)
    }
}
