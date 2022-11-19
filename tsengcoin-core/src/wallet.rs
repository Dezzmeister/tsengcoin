use ring::digest::{Context, SHA256};
use ripemd::{Ripemd160, Digest};

pub type Hash160 = [u8; 20];
pub type Hash256 = [u8; 32];

pub type Address = Hash160;

pub fn address_from_public_key(public_key: &Vec<u8>) -> Address {
    let mut context = Context::new(&SHA256);
    context.update(&public_key);
    let digest = context.finish();
    let sha256_hash = digest.as_ref();

    let mut hasher160 = Ripemd160::new();
    hasher160.update(sha256_hash);
    let result = hasher160.finalize().to_vec();

    let mut out = [0 as u8; 20];
    out.copy_from_slice(&result);

    out
}
