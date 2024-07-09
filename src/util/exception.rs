use std::backtrace::Backtrace;
use std::error::Error;
use std::fmt;
use std::io;

use tokio::sync::mpsc::error::SendError;
use tokio::task::JoinError;

pub enum Exception {
    ValidationError(String),
    ExternalError(String),
    Unexpected { message: String, trace: String },
}

impl Exception {
    pub fn unexpected<T>(error: T) -> Self
    where
        T: ToString,
    {
        Self::Unexpected {
            message: error.to_string(),
            trace: Backtrace::force_capture().to_string(),
        }
    }

    pub fn unexpected_with_context<T>(error: T, context: &str) -> Self
    where
        T: ToString,
    {
        Self::Unexpected {
            message: format!("error={}, context={}", error.to_string(), context),
            trace: Backtrace::force_capture().to_string(),
        }
    }
}

impl fmt::Debug for Exception {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Exception::ValidationError(message) => write!(f, "{}", message),
            Exception::ExternalError(message) => write!(f, "{}", message),
            Exception::Unexpected { message, trace } => write!(f, "{}\ntrace:\n{}", message, trace),
        }
    }
}

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl Error for Exception {}

impl From<io::Error> for Exception {
    fn from(err: io::Error) -> Self {
        Exception::unexpected(err)
    }
}

impl From<JoinError> for Exception {
    fn from(err: JoinError) -> Self {
        Exception::unexpected(err)
    }
}

impl<T> From<SendError<T>> for Exception {
    fn from(err: SendError<T>) -> Self {
        Exception::unexpected(err)
    }
}
