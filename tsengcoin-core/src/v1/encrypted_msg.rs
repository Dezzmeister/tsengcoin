use std::{error::Error, net::SocketAddr};

use base58check::{ToBase58Check, FromBase58Check};
use lazy_static::lazy_static;
use regex::Regex;
use ring::{aead::{NonceSequence, Nonce, UnboundKey, AES_256_GCM, SealingKey, BoundKey, Aad, OpeningKey}, error::Unspecified};
use serde::{Serialize, Deserialize};

use crate::wallet::Address;

use super::{transaction::{Transaction, get_p2pkh_addr, TxnOutput, get_p2pkh_sender}, state::State};

const B58C_VERSION_PREFIX: u8 = 0x07;

/// An encrypted request made on the blockchain instead of over the network. The two parties must 
/// perform a Diffie-Hellman key exchange first in order to determine a shared secret. The shared secret
/// is used to encrypt and decrypt these requests.
#[derive(Serialize, Deserialize)]
pub enum ChainRequest {
    FindMeAt(FindMeAtReq),
}

#[derive(Serialize, Deserialize)]
pub struct EncryptedChainRequest {
    pub ciphertext: Vec<u8>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FindMeAtReq {
    pub addr: SocketAddr,
}

pub struct NonceGen {
    current: u128,
    start: u128
}

impl NonceGen {
    fn new(start: [u8; 12]) -> Self {
        let mut nonce_bytes = [0 as u8; 16];
        nonce_bytes[4..].copy_from_slice(&start);

        let start: u128 = u128::from_be_bytes(nonce_bytes);

        Self {
            start,
            current: start.wrapping_add(1)
        }
    }
}

impl NonceSequence for NonceGen {
    fn advance(&mut self) -> Result<Nonce, Unspecified> {
        let prev = self.current;

        if prev == self.start {
            return Err(Unspecified);
        }

        self.current = prev.wrapping_add(1);

        Ok(Nonce::assume_unique_for_key(prev.to_be_bytes()[4..].try_into().unwrap()))
    }
}

pub fn handle_chain_request(req: ChainRequest, sender: Address, state: &mut State) -> Result<(), Box<dyn Error>> {
    match req {
        ChainRequest::FindMeAt(req) => handle_find_me_at(req, sender, state),
    }
}

fn handle_find_me_at(req: FindMeAtReq, _sender: Address, _state: &mut State) -> Result<(), Box<dyn Error>> {
    println!("Received \"FindMe\": {:#?}", req);

    Ok(())
}

pub fn make_sealing_key(secret: &[u8; 32], nonce_seed: [u8; 12]) -> Result<SealingKey<NonceGen>, Box<dyn Error>> {
    let unbound_key = UnboundKey::new(&AES_256_GCM, secret).map_err(|_| "Failed to create unbound key")?;
    
    Ok(SealingKey::new(unbound_key, NonceGen::new(nonce_seed)))
}

pub fn make_opening_key(secret: &[u8; 32], nonce_seed: [u8; 12]) -> Result<OpeningKey<NonceGen>, Box<dyn Error>> {
    let unbound_key = UnboundKey::new(&AES_256_GCM, secret).map_err(|_| "Failed to create unbound key")?;
    
    Ok(OpeningKey::new(unbound_key, NonceGen::new(nonce_seed)))
}

pub fn encrypt_request(req: ChainRequest, sealing: &mut SealingKey<NonceGen>) -> Result<EncryptedChainRequest, Box<dyn Error>> {
    let mut data = bincode::serialize(&req)?;
    sealing.seal_in_place_append_tag(Aad::empty(), &mut data).map_err(|_| "Failed to encrypt request")?;

    Ok(EncryptedChainRequest {
        ciphertext: data
    })
}

pub fn decrypt_request(req: EncryptedChainRequest, opening: &mut OpeningKey<NonceGen>) -> Result<ChainRequest, Box<dyn Error>> {
    let mut data = req.ciphertext;

    let decrypted_bytes = opening.open_in_place(Aad::empty(), &mut data).map_err(|_| "Failed to decrypt chat request")?;
    let chat_request: ChainRequest = bincode::deserialize(decrypted_bytes)?;

    Ok(chat_request)
}

pub fn req_to_b58c(req: &EncryptedChainRequest) -> Result<String, Box<dyn Error>> {
    let bytes = bincode::serialize(req)?;
    Ok(bytes.to_base58check(B58C_VERSION_PREFIX))
}

pub fn b58c_to_req(b58c: &str) -> Result<EncryptedChainRequest, Box<dyn Error>> {
    let (version, bytes) = b58c.from_base58check().map_err(|_| "Invalid base58check")?;

    if version != B58C_VERSION_PREFIX {
        return Err("Invalid base58check version")?;
    }

    let enc_req: EncryptedChainRequest = bincode::deserialize(&bytes)?;
    Ok(enc_req)
}

pub fn enc_req_meta(req: &EncryptedChainRequest) -> Result<String, Box<dyn Error>> {
    Ok(format!("ENC {}", req_to_b58c(req)?))
}

pub fn is_enc_req(txn: &Transaction) -> bool {
    if txn.outputs.len() > 2 {
        return false;
    }

    lazy_static!{
        static ref RE: Regex = Regex::new(r"ENC (\d|[a-z]|[A-Z])+").unwrap();
    }

    RE.is_match(&txn.meta)
}

pub fn decompose_enc_req(txn: &Transaction) -> Option<EncryptedChainRequest> {
    let items = txn.meta.split(" ").collect::<Vec<&str>>();

    match b58c_to_req(&items[1]) {
        Ok(req) => Some(req),
        Err(_) => None
    }
}

/// Assumes that the transaction has already been determined to be an encrypted request
pub fn is_enc_req_to_me(txn: &Transaction, state: &State) -> bool {
    let sender = match get_p2pkh_sender(txn, state) {
        Some(data) => data,
        None => return false
    };

    let outputs = &txn.outputs
        .iter()
        .filter(|o| {
            let dest = get_p2pkh_addr(&o.lock_script.code);
            match dest {
                None => false,
                Some(addr) => addr != sender
            }
        })
        .collect::<Vec<&TxnOutput>>();
    
    if outputs.len() != 1 {
        return false;
    }

    match get_p2pkh_addr(&outputs[0].lock_script.code) {
        None => false,
        Some(addr) => addr == state.address
    }
}
