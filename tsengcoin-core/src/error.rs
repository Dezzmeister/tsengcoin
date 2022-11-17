use std::error::{Error as StdError, self};
use std::fmt;

use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, Error>;

pub type Error = Box<ErrorKind>;

#[derive(Debug, Serialize, Deserialize)]
pub enum ErrorKind {
    InvalidTransactionScriptToken(String),
    ScriptStackUnderflow,
    InvalidTokenType,
    IntegerOverflow,
}

impl StdError for ErrorKind {
    fn description(&self) -> &str {
        match *self {
            ErrorKind::InvalidTransactionScriptToken(_) => "Bad token in transaction script",
            ErrorKind::ScriptStackUnderflow => "Stack underflow while executing transaction script",
            ErrorKind::InvalidTokenType => "Tried to perform an operation with a token of the wrong type",
            ErrorKind::IntegerOverflow => "Integer overflow"
        }
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match &*self {
            ErrorKind::InvalidTransactionScriptToken(token) => write!(fmt, "{}: token: {}", self.to_string(), token),
            ErrorKind::ScriptStackUnderflow => write!(fmt, "{}", self.to_string()),
            ErrorKind::InvalidTokenType => write!(fmt, "{}", self.to_string()),
            ErrorKind::IntegerOverflow => write!(fmt, "{}", self.to_string()),
        }
    }
}
