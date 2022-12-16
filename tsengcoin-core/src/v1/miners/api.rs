use std::{
    collections::HashMap,
    sync::{mpsc::Receiver, Mutex},
};

use chrono::{Utc, Duration};
use lazy_static::lazy_static;

use crate::{v1::{state::State, block::{RawBlock, MAX_TRANSACTION_FIELD_SIZE, make_merkle_root, RawBlockHeader}, transaction::{coinbase_size_estimate, make_coinbase_txn, Transaction, compute_fee}, VERSION}, wallet::Hash256};

/// Update the hashes per sec metric every 5 seconds by default
pub const HASH_PER_SEC_INTERVAL: i64 = 5;

lazy_static! {
    /// Poll the MinerMessage receiver every 5 seconds
    pub static ref POLL_INTERVAL: Duration = Duration::seconds(5);
}

pub type MineFunc = fn(&Mutex<State>, Receiver<MinerMessage>);

pub enum MinerMessage {
    /// The argument here is the number of new transactions added to the pending pool
    NewTransactions(usize),
    /// The arguments are:
    ///     1. The hash of the newly added block
    ///     2. Whether the pending transaction pool has changed
    NewBlock(Hash256, bool),
    // The argument is the new difficulty target
    NewDifficulty(Hash256),
}

/// Assumes that the miner name is a valid miner.
#[allow(unused_variables)]
pub fn start_miner(
    state_mut: &Mutex<State>,
    miner_receiver: Receiver<MinerMessage>,
    miner_name: &str,
) {
    let miner_map = make_miner_map();
    let mine_func = miner_map.get(miner_name).unwrap();

    mine_func(state_mut, miner_receiver);
}

pub fn has_any_miners() -> bool {
    let map = make_miner_map();

    !map.is_empty()
}

pub fn num_miners() -> usize {
    make_miner_map().len()
}

pub fn has_miner(name: &str) -> bool {
    make_miner_map().contains_key(name)
}

pub fn miners() -> Vec<String> {
    let map = make_miner_map();
    map.keys().map(|k| k.to_owned()).collect::<Vec<String>>()
}

#[allow(unused_mut)]
fn make_miner_map() -> HashMap<String, MineFunc> {
    let mut out: HashMap<String, MineFunc> = HashMap::new();

    #[cfg(feature = "cuda_miner")]
    {
        use super::cuda::mine;
        out.insert(String::from("cuda"), mine);
    }

    #[cfg(feature = "cl_miner")]
    {
        use super::cl::mine;
        out.insert(String::from("cl"), mine);
    }

    out
}

pub fn make_raw_block(state_mut: &Mutex<State>) -> RawBlock {
    let state = state_mut.lock().unwrap();
    let txns = state.pending_txns.clone();
    let (mut best_txns, fees) = pick_best_transactions(&txns, &state, coinbase_size_estimate());
    let coinbase = make_coinbase_txn(&state.address, String::from(""), fees, rand::random());

    let mut block_txns = vec![coinbase];
    block_txns.append(&mut best_txns);

    let prev_hash = state.blockchain.top_hash(0);
    let difficulty_target = state.blockchain.current_difficulty();

    let merkle_root = make_merkle_root(&block_txns);
    let header = RawBlockHeader {
        version: VERSION,
        prev_hash,
        merkle_root,
        timestamp: Utc::now().timestamp().try_into().unwrap(),
        difficulty_target,
        nonce: [0; 32],
    };

    RawBlock {
        header,
        transactions: block_txns,
    }
}

/// The problem here is to pick which transactions we will include in a block. Generally we want to maximize
/// the total fees while staying under the block size limit. This is the knapsack problem, and it is NP hard -
/// so rather than deal with it here we just take as many transactions as we can fit regardless of fee. We could take
/// a greedy approach to this problem and take the transactions with the highest fees, but then we would have to ensure that
/// we don't leave any dependency transactions behind. We chose not to deal with this because the network is small
/// and there won't be enough transactions to even approach the block size limit.
pub fn pick_best_transactions(
    txns: &[Transaction],
    state: &State,
    coinbase_size: usize,
) -> (Vec<Transaction>, u64) {
    let mut out: Vec<Transaction> = vec![];
    let mut size: usize = coinbase_size;
    let mut fees: u64 = 0;

    for txn in txns {
        let txn_size = txn.size();

        if (txn_size + size) > MAX_TRANSACTION_FIELD_SIZE {
            continue;
        }

        let fee = compute_fee(txn, state);
        out.push(txn.clone());
        size += txn_size;
        fees += fee;
    }

    (out, fees)
}

pub fn randomize(bytes: &mut [u8]) {
    for i in 0..bytes.len() {
        bytes[i] = rand::random();
    }
}

pub fn find_winner(nonces: &[u8], hashes: &[u8], difficulty: &Hash256) -> Option<(Hash256, Hash256)> {
    for i in 0..(nonces.len() / 32) {
        let t = i * 32;
        let hash: &[u8; 32] = hashes[t..(t + 32)].try_into().unwrap();

        if hash < difficulty {
            let nonce: [u8; 32] = nonces[t..(t + 32)].try_into().unwrap();

            return Some((nonce, hash.to_owned()));
        }
    }

    None
}
