pub mod miners;

pub mod block;
pub mod block_verify;
pub mod block_verify_error;
pub mod chain_request;
pub mod encrypted_msg;
pub mod net;
pub mod request;
pub mod response;
pub mod state;
pub mod transaction;
pub mod txn_verify;
pub mod txn_verify_error;

pub const VERSION: u32 = 1;
