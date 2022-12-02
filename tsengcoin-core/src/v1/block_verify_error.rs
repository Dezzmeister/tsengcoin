use std::error::{Error as StdError, self};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::wallet::Hash256;

use super::txn_verify_error::TxnVerifyError;

pub type BlockVerifyResult<T> = std::result::Result<T, BlockVerifyError>;

pub type BlockVerifyError = Box<ErrorKind>;

#[derive(Debug, Serialize, Deserialize)]
pub enum ErrorKind {
    IncorrectDifficulty,
    FailedProofOfWork,
    InvalidHeaderHash,
    OldBlock,
    TooLarge(usize, usize),
    EmptyBlock,
    TxnError(TxnVerifyError, Hash256),
    OrphanTxn(Hash256),
    InvalidCoinbase,
    InvalidCoinbaseAmount(u64, u64),
    InvalidMerkleRoot
}

impl StdError for ErrorKind {
    fn description(&self) -> &str {
        match *self {
            ErrorKind::IncorrectDifficulty => "Block difficulty is incorrect",
            ErrorKind::FailedProofOfWork => "Block hash is not low enough",
            ErrorKind::InvalidHeaderHash => "Block header hash is incorrect",
            ErrorKind::OldBlock => "Block header timestamp is out of date",
            ErrorKind::TooLarge(_, _) => "Block is too big",
            ErrorKind::EmptyBlock => "Block has no transactions",
            ErrorKind::TxnError(_, _) => "Invalid transaction in block",
            ErrorKind::OrphanTxn(_) => "Orphan transaction in block",
            ErrorKind::InvalidCoinbase => "Invalid coinbase transaction",
            ErrorKind::InvalidCoinbaseAmount(_, _) => "Invalid coinbase transaction amount",
            ErrorKind::InvalidMerkleRoot => "Invalid merkle root"
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
            ErrorKind::IncorrectDifficulty => write!(fmt, "{}", self.description()),
            ErrorKind::FailedProofOfWork => write!(fmt, "{}", self.description()),
            ErrorKind::InvalidHeaderHash => write!(fmt, "{}", self.description()),
            ErrorKind::OldBlock => write!(fmt, "{}", self.description()),
            ErrorKind::TooLarge(max_size, actual_size) => write!(fmt, "{}: max size is {}B, block is {}B", self.description(), max_size, actual_size),
            ErrorKind::EmptyBlock => write!(fmt, "{}", self.description()),
            ErrorKind::TxnError(err, txn) => write!(fmt, "{}: error: {}, txn: {}", self.description(), err.to_string(), hex::encode(txn)),
            ErrorKind::OrphanTxn(txn) => write!(fmt, "{}: txn: {}", self.description(), hex::encode(txn)),
            ErrorKind::InvalidCoinbase => write!(fmt, "{}", self.description()),
            ErrorKind::InvalidCoinbaseAmount(exp, actual) => write!(fmt, "{}: expected: {}, actual: {}", self.description(), exp, actual),
            ErrorKind::InvalidMerkleRoot => write!(fmt, "{}", self.description())
        }
    }
}
