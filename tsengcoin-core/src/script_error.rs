use std::{
    error::{self, Error as StdError},
    fmt,
};

use serde::{Deserialize, Serialize};

pub type ScriptResult<T> = std::result::Result<T, ScriptError>;

pub type ScriptError = Box<ErrorKind>;

#[derive(Debug, Serialize, Deserialize)]
pub enum ErrorKind {
    ScriptTooLong(usize, usize),
    InvalidScriptToken(String),
    ScriptStackOverflow,
    ScriptStackUnderflow,
    InvalidTokenType,
    IntegerOverflow,
    EqualVerifyFailed,
}

impl StdError for ErrorKind {
    fn description(&self) -> &str {
        match *self {
            ErrorKind::ScriptTooLong(_, _) => "Transaction script is too long",
            ErrorKind::InvalidScriptToken(_) => "Bad token in transaction script",
            ErrorKind::ScriptStackUnderflow => "Stack underflow",
            ErrorKind::ScriptStackOverflow => "Stack overflow",
            ErrorKind::InvalidTokenType => {
                "Tried to perform an operation with a token of the wrong type"
            }
            ErrorKind::IntegerOverflow => "Integer overflow",
            ErrorKind::EqualVerifyFailed => "Expected two tokens to be equal",
        }
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
}

impl fmt::Display for ErrorKind {
    #[allow(deprecated)]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match &*self {
            ErrorKind::ScriptTooLong(max_len, actual_len) => write!(
                fmt,
                "{}: Max length: {}B, actual length: {}B",
                self.description(),
                max_len,
                actual_len
            ),
            ErrorKind::InvalidScriptToken(token) => {
                write!(fmt, "{}: token: {}", self.description(), token)
            }
            ErrorKind::ScriptStackUnderflow => write!(fmt, "{}", self.description()),
            ErrorKind::ScriptStackOverflow => write!(fmt, "{}", self.description()),
            ErrorKind::InvalidTokenType => write!(fmt, "{}", self.description()),
            ErrorKind::IntegerOverflow => write!(fmt, "{}", self.description()),
            ErrorKind::EqualVerifyFailed => write!(fmt, "{}", self.description()),
        }
    }
}
