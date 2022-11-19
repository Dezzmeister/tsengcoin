use std::error::{Error as StdError, self};
use std::fmt;

use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, Error>;

pub type Error = Box<ErrorKind>;

#[derive(Debug, Serialize, Deserialize)]
pub enum ErrorKind {
    ScriptTooLong(usize, usize),
    InvalidScriptToken(String),
    ScriptStackOverflow,
    ScriptStackUnderflow,
    InvalidTokenType,
    IntegerOverflow,
    EqualVerifyFailed
}

impl StdError for ErrorKind {
    fn description(&self) -> &str {
        match *self {
            ErrorKind::ScriptTooLong(_, _) => "Transaction script is too long",
            ErrorKind::InvalidScriptToken(_) => "Bad token in transaction script",
            ErrorKind::ScriptStackUnderflow => "Stack underflow",
            ErrorKind::ScriptStackOverflow => "Stack overflow",
            ErrorKind::InvalidTokenType => "Tried to perform an operation with a token of the wrong type",
            ErrorKind::IntegerOverflow => "Integer overflow",
            ErrorKind::EqualVerifyFailed => "Expected two tokens to be equal"
        }
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match &*self {
            ErrorKind::ScriptTooLong(max_len, actual_len) => write!(fmt, "{}: Max length: {}B, actual length: {}B", self.to_string(), max_len, actual_len),
            ErrorKind::InvalidScriptToken(token) => write!(fmt, "{}: token: {}", self.to_string(), token),
            ErrorKind::ScriptStackUnderflow => write!(fmt, "{}", self.to_string()),
            ErrorKind::ScriptStackOverflow => write!(fmt, "{}", self.to_string()),
            ErrorKind::InvalidTokenType => write!(fmt, "{}", self.to_string()),
            ErrorKind::IntegerOverflow => write!(fmt, "{}", self.to_string()),
            ErrorKind::EqualVerifyFailed => write!(fmt, "{}", self.to_string()),
        }
    }
}
