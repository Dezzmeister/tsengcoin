use std::error::{Error as StdError, self};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::script_error::ScriptError;
use crate::wallet::Hash256;

use super::block::MAX_BLOCK_SIZE;
use super::transaction::{MAX_TXN_AMOUNT, MIN_TXN_FEE};

pub type TxnVerifyResult<T> = std::result::Result<T, TxnVerifyError>;

pub type TxnVerifyError = Box<ErrorKind>;

#[derive(Debug, Serialize, Deserialize)]
pub enum ErrorKind {
    EmptyInputs,
    EmptyOutputs,
    TooLarge,
    OutOfRange(u64),
    Coinbase,
    InvalidUTXOIndex,
    Script(ScriptError),
    BadUnlockScript(Hash256, usize),
    Overspend(u64, u64),
    LowFee(u64),
    DoubleSpend(Hash256, usize),
    InvalidHash,
    ZeroOutput
}

impl StdError for ErrorKind {
    fn description(&self) -> &str {
        match *self {
            ErrorKind::EmptyInputs => "Transaction has no inputs",
            ErrorKind::EmptyOutputs => "Transaction has no outputs",
            ErrorKind::TooLarge => "Transaction is too big",
            ErrorKind::OutOfRange(_) => "Transaction amount is out of range",
            ErrorKind::Coinbase => "Transaction input had zero hash. If this is a coinbase transaction, it should not be relayed",
            ErrorKind::InvalidUTXOIndex => "Transaction input references a UTXO that does not exist",
            ErrorKind::Script(_) => "Transaction script error",
            ErrorKind::BadUnlockScript(_, _) => "Unlocking script did not satisfy locking script requirements",
            ErrorKind::Overspend(_, _) => "Tried to spend more than total amount in inputs",
            ErrorKind::LowFee(_) => "Transaction fee is too low",
            ErrorKind::DoubleSpend(_, _) => "Transaction output has already been spent",
            ErrorKind::InvalidHash => "Transaction hash is invalid",
            ErrorKind::ZeroOutput => "Transaction has at least one output with zero TsengCoin"
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
            ErrorKind::EmptyInputs => write!(fmt, "{}", self.description()),
            ErrorKind::EmptyOutputs => write!(fmt, "{}", self.description()),
            ErrorKind::TooLarge => write!(fmt, "{}. Cannot exceed {} bytes", self.description(), MAX_BLOCK_SIZE),
            ErrorKind::OutOfRange(val) => write!(fmt, "{}. Max is {} TsengCoin, received {}", self.description(), MAX_TXN_AMOUNT, val),
            ErrorKind::Coinbase => write!(fmt, "{}", self.description()),
            ErrorKind::InvalidUTXOIndex => write!(fmt, "{}", self.description()),
            ErrorKind::Script(err) => write!(fmt, "{}: {}", self.description(), err),
            ErrorKind::BadUnlockScript(hash, output_idx) => write!(fmt, "{}: input transaction {}, output {}", self.description(), hex::encode(hash), output_idx),
            ErrorKind::Overspend(input_amt, output_amt) => write!(fmt, "{}: tried to spend {} when only {} provided as input", self.description(), output_amt, input_amt),
            ErrorKind::LowFee(fee) => write!(fmt, "{}: Tried to spend fee of {}, minimum fee is {}", self.description(), fee, MIN_TXN_FEE),
            ErrorKind::DoubleSpend(hash, output_idx) => write!(fmt, "{}: hash: {}, output index: {}", self.description(), hex::encode(hash), output_idx),
            ErrorKind::InvalidHash => write!(fmt, "{}", self.description()),
            ErrorKind::ZeroOutput => write!(fmt, "{}", self.description())
        }
    }
}
