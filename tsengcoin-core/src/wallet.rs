use std::error::Error;

use ring::digest::{Context, SHA256};
use ripemd::{Ripemd160, Digest};
use base58check::{ToBase58Check, FromBase58Check, FromBase58CheckError};

pub type Hash160 = [u8; 20];
pub type Hash256 = [u8; 32];

pub type Address = Hash160;

const B58C_VERSION_PREFIX: u8 = 0x01;

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

pub fn address_to_b58c(address: &Vec<u8>) -> String {
    address.to_base58check(B58C_VERSION_PREFIX)
}

pub fn b58c_to_address(b58c: String) -> Result<Address, Box<dyn Error>> {
    let res = b58c.from_base58check();

    match res {
        Err(FromBase58CheckError::InvalidChecksum) => Err("Invalid checksum")?,
        Err(FromBase58CheckError::InvalidBase58(_)) => Err("Invalid base58")?,
        Ok((version, _)) if version != B58C_VERSION_PREFIX => Err("Invalid version")?,
        Ok((_, bytes)) => {
            let offset = 20 - bytes.len();
            let mut out = [0 as u8; 20];

            // Pad 20-byte integer with correct number of zeros
            out[offset..].copy_from_slice(&bytes);

            Ok(out)
        },
    }
}
