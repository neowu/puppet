use std::backtrace::Backtrace;
use std::error::Error;
use std::fmt;

pub struct Exception {
    message: String,
    trace: String,
}

impl Exception {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
            trace: Backtrace::capture().to_string(),
        }
    }
}

impl fmt::Debug for Exception {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Exception: {}\ntrace:\n{}", self.message, self.trace)
    }
}

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for Exception {}
