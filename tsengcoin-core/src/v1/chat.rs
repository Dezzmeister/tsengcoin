use std::{collections::HashMap, error::Error};

use crate::{wallet::{Address, address_to_b58c, b58c_to_address}, v1::transaction::get_p2pkh_addr};

use rand_core::OsRng;
use regex::Regex;
use ring::signature::KeyPair;
use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};
use lazy_static::lazy_static;

use super::{transaction::{Transaction, sign_txn, make_p2pkh_unlock, TxnInput, UnhashedTransaction, hash_txn, make_single_p2pkh_txn, TxnOutput}, state::State, VERSION, txn_verify::verify_transaction};

pub struct ChatState {
    /// Pending Diffie-Hellman key exchanges - we have shared our public key but they haven't given us
    /// theirs yet
    pub pending_dh: HashMap<Address, EphemeralSecret>,
    /// Completed DH key exchanges, both public keys have been shared and we now have the same shared secret
    pub completed_dh: HashMap<Address, SharedSecret>,
    /// Maps addresses to readable names
    pub aliases: HashMap<Address, String>
}

impl ChatState {
    pub fn get_name(&self, addr: Address) -> String {
        match self.aliases.get(&addr) {
            Some(name) => name.clone(),
            None => address_to_b58c(&addr.to_vec())
        }
    }

    pub fn get_address(&self, name: String) -> Result<Address, Box<dyn Error>> {
        for (addr, alias) in self.aliases.iter() {
            if *alias == name {
                return Ok(*addr);
            }
        }

        b58c_to_address(name)
    }
}

impl std::fmt::Debug for ChatState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatState")
            .finish()
    }
}

/// Checks the pending Diffie-Hellman map and returns true if the caller should proceed with
/// a Diffie-Hellman response request. If we initiated a DH key exchange, we don't want to send
/// a DH response back - we want to send an encrypted request
pub fn check_pending_dh(your_pubkey: PublicKey, sender: Address, state: &mut State) -> bool {
    if !state.chat.pending_dh.contains_key(&sender) {
        return true;
    }
    
    let my_secret = state.chat.pending_dh.remove(&sender).unwrap();
    let shared_secret = my_secret.diffie_hellman(&your_pubkey);
    
    state.chat.completed_dh.insert(sender, shared_secret);

    false
}

pub fn make_chat_response_req(txn: &Transaction, state: &mut State) -> Result<(Transaction, Address), Box<dyn Error>> {
    let (your_pubkey, your_address) = decompose_chat_req(txn).unwrap();
    let req_amount = get_chat_req_amount(txn, state.address).unwrap();

    let my_secret = EphemeralSecret::new(OsRng);
    let my_pubkey = PublicKey::from(&my_secret);

    let (mut unsigned_txn, input_utxos, outputs) = make_single_p2pkh_txn(your_address, req_amount, 1, state)?;
    unsigned_txn.meta = chat_req_meta(my_pubkey, state.address);

    let sig = sign_txn(&unsigned_txn, &state.keypair)?;
    let pubkey = state.keypair.public_key().as_ref().to_vec();
    let unlock_script = make_p2pkh_unlock(sig, pubkey);
    let txn_inputs =
        input_utxos
            .iter()
            .map(|c| {
                TxnInput {
                    txn_hash: c.txn,
                    output_idx: c.output,
                    unlock_script: unlock_script.clone(),
                }
            })
            .collect::<Vec<TxnInput>>();
    let unhashed = UnhashedTransaction {
        version: VERSION,
        inputs: txn_inputs,
        outputs,
        meta: unsigned_txn.meta,
    };

    let hash = hash_txn(&unhashed)?;
    let full_txn = unhashed.to_hashed(hash);

    match verify_transaction(full_txn.clone(), state) {
        Ok(_) => {
            let shared_secret = my_secret.diffie_hellman(&your_pubkey);
            state.chat.completed_dh.insert(your_address, shared_secret);

            Ok((full_txn, your_address))
        },
        Err(err) => {
            Err(format!("Error verifying chat request transaction: {}", err))?
        }
    }
}

pub fn make_chat_req(dest: Address, req_amount: u64, fee: u64, state: &mut State) -> Result<Transaction, Box<dyn Error>> {
    let secret = EphemeralSecret::new(OsRng);
    let public = PublicKey::from(&secret);

    let (mut unsigned_txn, input_utxos, outputs) = make_single_p2pkh_txn(dest, req_amount, fee, state)?;
    unsigned_txn.meta = chat_req_meta(public, state.address);

    let sig = sign_txn(&unsigned_txn, &state.keypair)?;
    let pubkey = state.keypair.public_key().as_ref().to_vec();
    let unlock_script = make_p2pkh_unlock(sig, pubkey);
    let txn_inputs =
        input_utxos
            .iter()
            .map(|c| {
                TxnInput {
                    txn_hash: c.txn,
                    output_idx: c.output,
                    unlock_script: unlock_script.clone(),
                }
            })
            .collect::<Vec<TxnInput>>();
    let unhashed = UnhashedTransaction {
        version: VERSION,
        inputs: txn_inputs,
        outputs,
        meta: unsigned_txn.meta,
    };

    let hash = hash_txn(&unhashed)?;
    let full_txn = unhashed.to_hashed(hash);

    match verify_transaction(full_txn.clone(), state) {
        Ok(_) => {
            state.chat.pending_dh.insert(dest, secret);
            Ok(full_txn)
        },
        Err(err) => {
            Err(format!("Error verifying chat request transaction: {}", err))?
        }
    }
}


pub fn chat_req_meta(pubkey: PublicKey, sender_address: Address) -> String {
    let encoded = hex::encode(pubkey.as_bytes());

    format!("DH {} {}", encoded, hex::encode(sender_address))
}


pub fn decompose_chat_req(txn: &Transaction) -> Option<(PublicKey, Address)> {
    let items = txn.meta.split(" ").collect::<Vec<&str>>();
    let pubkey_vec = match hex::decode(&items[1]) {
        Ok(bytes) => bytes,
        Err(_) => return None
    };

    let sender_address_vec = match hex::decode(&items[2]) {
        Ok(bytes) => bytes,
        Err(_) => return None
    };

    let mut pubkey: [u8; 32] = [0; 32];
    let mut sender_address: [u8; 20] = [0; 20];

    pubkey[(32 - pubkey_vec.len())..].copy_from_slice(&pubkey_vec);
    sender_address[(20 - sender_address_vec.len())..].copy_from_slice(&sender_address_vec);

    Some((PublicKey::from(pubkey), sender_address))
}

pub fn is_chat_req(txn: &Transaction) -> bool {
    if txn.outputs.len() > 2 {
        return false;
    }

    lazy_static!{
        static ref RE: Regex = Regex::new(r"DH (\d|[a-f]|[A-F]){64} (\d|[a-f]|[A-F]){40}").unwrap();
    }

    RE.is_match(&txn.meta)
}

pub fn is_chat_req_to_me(txn: &Transaction, my_address: Address) -> bool {
    let (_, sender) = decompose_chat_req(txn).unwrap();

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
        Some(addr) => addr == my_address
    }
}

fn get_chat_req_amount(txn: &Transaction, my_address: Address) -> Option<u64> {
    for output in &txn.outputs {
        let output_dest = get_p2pkh_addr(&output.lock_script.code);
        if output_dest == Some(my_address) {
            return Some(output.amount);
        }
    }

    None
}
