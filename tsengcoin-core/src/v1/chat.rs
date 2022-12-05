use std::{collections::HashMap, error::Error};

use crate::{wallet::{Address, address_to_b58c, b58c_to_address}, v1::transaction::get_p2pkh_addr};

use rand_core::OsRng;
use regex::Regex;
use ring::{signature::KeyPair, aead::{SealingKey, OpeningKey}};
use x25519_dalek::{EphemeralSecret, PublicKey};
use lazy_static::lazy_static;

use super::{transaction::{Transaction, sign_txn, make_p2pkh_unlock, TxnInput, UnhashedTransaction, hash_txn, make_single_p2pkh_txn, TxnOutput, get_p2pkh_sender}, state::State, VERSION, txn_verify::verify_transaction, encrypted_msg::{ChainRequest, encrypt_request, enc_req_meta, FindMeAtReq, make_sealing_key, make_opening_key, NonceGen, EncryptedChainRequest, decrypt_request}};

pub struct ChatState {
    /// Pending Diffie-Hellman key exchanges - we have shared our public key but they haven't given us
    /// theirs yet
    pub pending_dh: HashMap<Address, EphemeralSecret>,
    /// Maps addresses to readable names
    pub aliases: HashMap<Address, String>,
    /// Keys used for encrypting/decrypting messages after a handshake has been completed
    pub keys: HashMap<Address, Keypair>,
}

pub struct Keypair {
    pub sealing: SealingKey<NonceGen>,
    pub opening: OpeningKey<NonceGen>,
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

    pub fn decrypt_from_sender(&mut self, enc_req: EncryptedChainRequest, sender: Address) -> Result<ChainRequest, Box<dyn Error>> {
        if !self.keys.contains_key(&sender) {
            return Err(format!("No encrypted connection set up with {}", self.get_name(sender)))?;
        }

        let keypair = self.keys.get_mut(&sender).unwrap();
        let chain_req = decrypt_request(enc_req, &mut keypair.opening)?;

        Ok(chain_req)
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
pub fn check_pending_dh(your_pubkey: PublicKey, sender: Address, state: &mut State) -> Result<bool, Box<dyn Error>> {
    if !state.chat.pending_dh.contains_key(&sender) {
        return Ok(true);
    }
    
    let my_secret = state.chat.pending_dh.remove(&sender).unwrap();
    let shared_secret = my_secret.diffie_hellman(&your_pubkey);

    let secret = shared_secret.as_bytes();

    let nonce_seed: [u8; 12] = [0; 12];
    let sealing_key = make_sealing_key(secret, nonce_seed)?;
    let opening_key = make_opening_key(secret, nonce_seed)?;
    
    let keypair = Keypair {
        sealing: sealing_key,
        opening: opening_key
    };

    state.chat.keys.insert(sender, keypair);

    Ok(false)
}

/// Constructed a transaction containing an encrypted "find me" request. At this point the Diffie-Hellman
/// key exchange has been completed, so both parties have a shared secret. Because of the existing
/// ECDSA signatures used in P2PKH transactions, both parties are sure that their communications have
/// not been intercepted and there is no man in the middle.
pub fn make_enc_find_me_req(dest: Address, state: &mut State) -> Result<Transaction, Box<dyn Error>> {
    let req = ChainRequest::FindMeAt(FindMeAtReq {
        addr: state.remote_addr_me.unwrap(),
    });
    let keypair = match state.chat.keys.get_mut(&dest) {
        None => return Err("Can't send encrypted request before performing Diffie Hellman key exchange")?,
        Some(key) => key
    };

    let enc_req = encrypt_request(req, &mut keypair.sealing)?;

    let (mut unsigned_txn, input_utxos, outputs) = make_single_p2pkh_txn(dest, 1, 1, state)?;
    unsigned_txn.meta = enc_req_meta(&enc_req)?;

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
            Ok(full_txn)
        },
        Err(err) => {
            Err(format!("Error verifying encrypted request transaction: {}", err))?
        }
    }
}

pub fn make_chat_response_req(txn: &Transaction, state: &mut State) -> Result<(Transaction, Address), Box<dyn Error>> {
    let your_pubkey = decompose_chat_req(txn).unwrap();
    let your_address = get_p2pkh_sender(txn, state).unwrap();
    let req_amount = get_chat_req_amount(txn, state.address).unwrap();

    let my_secret = EphemeralSecret::new(OsRng);
    let my_pubkey = PublicKey::from(&my_secret);

    let (mut unsigned_txn, input_utxos, outputs) = make_single_p2pkh_txn(your_address, req_amount, 1, state)?;
    unsigned_txn.meta = chat_req_meta(my_pubkey);

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
            let secret = shared_secret.as_bytes();
            let nonce_seed: [u8; 12] = [0; 12];
            let sealing_key = make_sealing_key(secret, nonce_seed)?;
            let opening_key = make_opening_key(secret, nonce_seed)?;

            let keypair = Keypair {
                sealing: sealing_key,
                opening: opening_key
            };

            state.chat.keys.insert(your_address, keypair);

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
    unsigned_txn.meta = chat_req_meta(public);

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

pub fn chat_req_meta(pubkey: PublicKey) -> String {
    let encoded = hex::encode(pubkey.as_bytes());

    format!("DH {}", encoded)
}

pub fn decompose_chat_req(txn: &Transaction) -> Option<PublicKey> {
    let items = txn.meta.split(" ").collect::<Vec<&str>>();
    let pubkey_vec = match hex::decode(&items[1]) {
        Ok(bytes) => bytes,
        Err(_) => return None
    };

    let mut pubkey: [u8; 32] = [0; 32];

    pubkey[(32 - pubkey_vec.len())..].copy_from_slice(&pubkey_vec);

    Some(PublicKey::from(pubkey))
}

pub fn is_chat_req(txn: &Transaction) -> bool {
    if txn.outputs.len() > 2 {
        return false;
    }

    lazy_static!{
        static ref RE: Regex = Regex::new(r"DH (\d|[a-f]|[A-F]){64}").unwrap();
    }

    RE.is_match(&txn.meta)
}

pub fn is_chat_req_to_me(txn: &Transaction, state: &State) -> bool {
    let sender = match get_p2pkh_sender(txn, state) {
        None => return false,
        Some(data) => data
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

fn get_chat_req_amount(txn: &Transaction, my_address: Address) -> Option<u64> {
    for output in &txn.outputs {
        let output_dest = get_p2pkh_addr(&output.lock_script.code);
        if output_dest == Some(my_address) {
            return Some(output.amount);
        }
    }

    None
}
