pub mod miners;

pub mod block;
pub mod transaction;
pub mod net;
pub mod request;
pub mod response;
pub mod state;
pub mod txn_verify_error;
pub mod block_verify_error;
pub mod txn_verify;
pub mod block_verify;
pub mod chat;
pub mod encrypted_msg;

pub const VERSION: u32 = 1;
