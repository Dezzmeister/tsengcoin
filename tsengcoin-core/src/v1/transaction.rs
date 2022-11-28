use regex::Regex;
use ring::{digest::{Context, SHA256}, signature::EcdsaKeyPair};
use serde::{Serialize, Deserialize};
use std::{mem::{size_of, size_of_val}, error::Error};
use lazy_static::lazy_static;

use crate::wallet::{Hash256, Address};

use super::{VERSION, state::State};

pub const BLOCK_REWARD: u64 = 1000;
pub const MAX_META_LENGTH: usize = 256;
/// Cannot send or receive more than 1bil TsengCoin at a time
pub const MAX_TXN_AMOUNT: u64 = 1_000_000_000;
/// Every transaction must give up at least 1 TsengCoin as a tx fee
pub const MIN_TXN_FEE: u64 = 1;

#[derive(Serialize, Deserialize, Clone)]
pub struct Transaction {
    /// Protocol version
    pub version: u32,
    /// Input transactions
    pub inputs: Vec<TxnInput>,
    /// Recipients
    pub outputs: Vec<TxnOutput>,
    /// Some metadata, use it to put messages on the blockchain. Max length [MAX_META_LENGTH]
    pub meta: String,
    /// Hash of all previous fields (an [UnhashedTransaction])
    pub hash: Hash256,
}

/// A transaction before signing. Meant to be serialized and signed that way. The inputs are not signed
/// because the signature will likely need to be provided in unlocking scripts for each input.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UnsignedTransaction {
    pub version: u32,
    pub outputs: Vec<TxnOutput>,
    /// Some metadata
    pub meta: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UnhashedTransaction {
    pub version: u32,
    pub inputs: Vec<TxnInput>,
    pub outputs: Vec<TxnOutput>,
    pub meta: String
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TxnOutput {
    pub amount: u64,
    pub lock_script: Script
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TxnInput {
    pub txn_hash: Hash256,
    pub output_idx: usize,
    pub unlock_script: Script,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ScriptType {
    TsengScript
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Script {
    pub code: String,
    pub script_type: ScriptType
}

/// Pool of unspent transaction outputs (UTXOs). UTXOs are updated whenever a new transaction is validated
/// or when a new block is accepted. UTXOs are also updated when the blockchain is unwound and previously
/// validated transactions are put back into the pending transaction pool.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UTXOPool {
    pub last_hash: Hash256,
    pub utxos: Vec<TransactionIndex>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransactionIndex {
    /// The block containing the transaction with unspent output. Will be None if
    /// the unspent output is in the pending transactions pool
    pub block: Option<Hash256>,
    /// The hash of the transaction containing the unspent output. This transaction must always exist,
    /// whether in a block or in the transaction pool.
    pub txn: Hash256,
    /// The indices of the unspent outputs in the given transaction. If this vector is ever empty,
    /// then the entire [TransactionIndex] should be removed from the UTXO pool.
    /// 
    /// Note that this is an array of indices into ANOTHER array
    pub outputs: Vec<usize>,
}

/// A pointer to a single UTXO
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UTXOWindow {
    pub block: Option<Hash256>,
    pub txn: Hash256,
    pub output: usize,
    pub amount: u64
}

impl Transaction {
    pub fn size(&self) -> usize {
        size_of_val(&self.version) +
        self.inputs.iter().fold(0, |a, e| a + e.size()) +
        self.outputs.iter().fold(0, |a, e| a + e.size()) +
        self.meta.len() +
        size_of::<usize>() +
        size_of_val(&self.hash)
    }
}

impl UnhashedTransaction {
    pub fn to_hashed(self, hash: Hash256) -> Transaction {
        Transaction {
            version: self.version,
            inputs: self.inputs,
            outputs: self.outputs,
            meta: self.meta,
            hash,
        }
    }
}

impl TxnOutput {
    pub fn size(&self) -> usize {
        size_of_val(&self.amount) + 
        self.lock_script.size()
    }
}

impl TxnInput {
    pub fn size(&self) -> usize {
        size_of_val(&self.txn_hash) + 
        size_of_val(&self.output_idx) + 
        self.unlock_script.size()
    }
}

impl Script {
    pub fn size(&self) -> usize {
        self.code.len() +
        size_of::<usize>() +
        size_of_val(&self.script_type)
    }
}

impl UTXOPool {
    pub fn find_txn_index<'a, T: PartialEq>(&'a self, txn: T) -> Option<&'a TransactionIndex> 
        where Hash256: PartialEq<T>
    {
        self.utxos.iter().find(|t| t.txn == txn)
    }

    /// Removes the UTXOs spent in the given transaction from the pool and adds UTXOs
    /// for the outputs of this transaction.
    /// Assumes that this is a valid transaction and all UTXOS are already in the pool.
    pub fn update_unconfirmed(&mut self, tx: &Transaction) {
        for input in &tx.inputs {
            let utxo_pos = self.utxos.iter().position(|u| u.txn == input.txn_hash).unwrap();
            let utxo = &mut self.utxos[utxo_pos];
            let output_pos = utxo.outputs.iter().position(|i| *i == input.output_idx).unwrap();

            utxo.outputs.remove(output_pos);
        }

        let txn_idx = TransactionIndex {
            block: None,
            txn: tx.hash,
            outputs: (0..tx.outputs.len()).collect::<Vec<usize>>(),
        };

        self.utxos.push(txn_idx);
    }
}

impl PartialEq for Transaction {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl PartialEq<Hash256> for Transaction {
    fn eq(&self, other: &Hash256) -> bool {
        self.hash == *other
    }
}

impl From<Transaction> for UnsignedTransaction {
    fn from(txn: Transaction) -> Self {
        Self {
            version: txn.version,
            outputs: txn.outputs,
            meta: txn.meta,
        }
    }
}

impl From<&Transaction> for UnsignedTransaction {
    fn from(txn: &Transaction) -> Self {
        Self {
            version: txn.version,
            outputs: txn.outputs.clone(),
            meta: txn.meta.clone(),
        }
    }
}

impl From<Transaction> for UnhashedTransaction {
    fn from(txn: Transaction) -> Self {
        Self {
            version: txn.version,
            inputs: txn.inputs,
            outputs: txn.outputs,
            meta: txn.meta
        }
    }
}

impl From<&Transaction> for UnhashedTransaction {
    fn from(txn: &Transaction) -> Self {
        Self {
            version: txn.version,
            inputs: txn.inputs.clone(),
            outputs: txn.outputs.clone(),
            meta: txn.meta.clone()
        }
    }
}

impl std::fmt::Debug for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transaction")
            .field("version", &self.version)
            .field("inputs", &self.inputs)
            .field("outputs", &self.outputs)
            .field("meta", &self.meta)
            .field("hash", &hex::encode(&self.hash))
            .finish()
    }
}

impl std::fmt::Debug for TxnInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxnInput")
            .field("txn_hash", &hex::encode(&self.txn_hash))
            .field("utxo_output_index", &self.output_idx)
            .field("unlock_script", &self.unlock_script)
            .finish()
    }
}

/// The coinbase transaction is the transaction in which a miner receives a block reward. The output amount
/// is the block reward plus the transaction fees.
pub fn make_coinbase_txn(winner: &Address, meta: String, fees: u64) -> Transaction {
    let input = TxnInput {
        txn_hash: [0; 32],
        output_idx: 0xFFFF_FFFF,
        unlock_script: Script {
            code: String::from(""),
            script_type: ScriptType::TsengScript,
        },
    };

    let output = TxnOutput {
        amount: BLOCK_REWARD + fees,
        lock_script: make_p2pkh_lock(&winner)
    };

    let mut out = Transaction {
        version: VERSION,
        inputs: vec![input],
        outputs: vec![output],
        meta,
        hash: [0; 32],
    };

    let unhashed: UnhashedTransaction = (&out).into();
    let unhashed_bytes = bincode::serialize(&unhashed).expect("Failed to make coinbase transaction hash");
    let mut context = Context::new(&SHA256);
    context.update(&unhashed_bytes);
    let digest = context.finish();
    let hash = digest.as_ref();

    out.hash.copy_from_slice(hash);

    out
}

/// Make a pay-to-public-key-hash locking script for the given public key hash (an address)
pub fn make_p2pkh_lock(address: &Address) -> Script {
    let address_hex = hex::encode(address);
    let script_text = format!("DUP HASH160 {} REQUIRE_EQUAL CHECKSIG", address_hex);

    Script {
        code: script_text,
        script_type: ScriptType::TsengScript,
    }
}

fn is_p2pkh_lock(code: &str) -> bool {
    lazy_static!{
        static ref RE: Regex = Regex::new(r"DUP HASH160 (\d|[a-f]|[A-F]){40} REQUIRE_EQUAL CHECKSIG").unwrap();
    };
    
    RE.is_match(code)
}

pub fn make_p2pkh_unlock(sig: Vec<u8>, pubkey: Vec<u8>) -> Script {
    let sig_hex = hex::encode(sig);
    let pubkey_hex = hex::encode(pubkey);
    let script_text = format!("{} {}", sig_hex, pubkey_hex);

    Script {
        code: script_text,
        script_type: ScriptType::TsengScript
    }
}

/// P2PKH transactions generated by the software must use the full 40-byte hex representation
/// of an address. Any leading zeroes are kept.
pub fn get_p2pkh_addr(code: &str) -> Option<Address> {
    lazy_static!{
        static ref RE: Regex = Regex::new(r"(\d|[a-f]|[A-F]){40}").unwrap();
    };

    if !is_p2pkh_lock(code) {
        return None;
    }

    let caps = 
        match RE.captures(code) {
            None => return None,
            Some(caps) => caps
        };

    let addr_vec = hex::decode(&caps[0]).unwrap();
    let mut out: Address = [0; 20];
    out[(20 - addr_vec.len())..].copy_from_slice(&addr_vec);

    Some(out)
}

/// Get the total unspent outputs for P2PKH transactions addressed to the given
/// recipient. P2PKH transactions are the most common type, and it is easy to determine the recipient
/// of a P2PKH transaction because the lock script will contain the recipient's address, so we can
/// have a function that will identify any P2PKH transactions addressed to the recipient.
pub fn p2pkh_utxos_for_addr(state: &State, addr: Address) -> Vec<UTXOWindow> {
    state.blockchain.utxo_pool.utxos
        .iter()
        // We will build our result by accumulating a vec of pointers to individual UTXO outputs (UTXOWindows)
        .fold(vec![] as Vec<UTXOWindow>, |mut a, u| {
            let txn = state.get_pending_or_confirmed_txn(u.txn).unwrap();
            let mut outputs = 
                u.outputs
                    .iter()
                    // Get the transaction output and keep the index
                    .map(|idx| (txn.outputs[*idx].clone(), idx))
                    // Filter any transaction outputs that we can't unlock with P2PKH
                    .filter(|(out, _)| {
                        let dest_addr = get_p2pkh_addr(&out.lock_script.code);
                        match dest_addr {
                            None => false,
                            Some(dest) => dest == addr
                        }
                    })
                    // Now we have transaction outputs which we can unlock. Convert these to UTXOWindows
                    // using the output index we saved earlier
                    .map(|(out, idx)| {
                        UTXOWindow {
                            block: u.block,
                            txn: u.txn,
                            output: *idx,
                            amount: out.amount
                        }
                    })
                    .collect::<Vec<UTXOWindow>>();

            a.append(&mut outputs);
            a
        })
}

/// Collect enough UTXOs to meet the required amount to make a transaction. If we don't have enough UTXOs
/// to meet the threshold, return None. We use a simple algorithm that takes transactions starting from 
/// the earliest UTXOs. This enables future optimizations in which the UTXO pool is calculated from 
/// a later block in the blockchain because all early transaction outputs have already been spent.
pub fn collect_enough_change(state: &State, addr: Address, threshold: u64) -> Option<Vec<UTXOWindow>> {
    let my_utxos = p2pkh_utxos_for_addr(state, addr);

    let mut amount = 0;
    let mut out: Vec<UTXOWindow> = vec![];

    for i in 0..my_utxos.len() {
        let utxo = &my_utxos[i];
        amount += utxo.amount;
        out.push(utxo.clone());

        if amount >= threshold {
            return Some(out);
        }
    }

    None
}

pub fn sign_txn(txn: &UnsignedTransaction, signer: &EcdsaKeyPair) -> Result<Vec<u8>, Box<dyn Error>> {
    let bytes = bincode::serialize(txn)?;
    let rng = ring::rand::SystemRandom::new();
    let sig = signer.sign(&rng, &bytes).expect("Failed to sign transaction");
    
    Ok(sig.as_ref().to_vec())
}

pub fn hash_txn(txn: &UnhashedTransaction) -> Result<Hash256, Box<dyn Error>> {
    let unhashed_bytes = bincode::serialize(txn)?;
    let mut context = Context::new(&SHA256);
    context.update(&unhashed_bytes);
    let digest = context.finish();
    let hash = digest.as_ref();

    let mut out = [0 as u8; 32];
    out.copy_from_slice(hash);

    Ok(out)
}
