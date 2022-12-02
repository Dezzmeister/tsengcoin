use std::mem::size_of_val;

use chrono::{DateTime, Utc, TimeZone, Duration};
use lazy_static::lazy_static;
use num_bigint::BigUint;
use num_traits::Zero;
use ring::digest::{Context, SHA256};
use serde::{Serialize, Deserialize};

use crate::{wallet::{Hash256, b58c_to_address}};

use super::{transaction::{Transaction, make_coinbase_txn, UTXOPool, build_utxos_from_confirmed}, block_verify::verify_block, state::State, txn_verify::check_pending_and_orphans};

/// Max size of a block in bytes
pub const MAX_BLOCK_SIZE: usize = 16384;

lazy_static!{
    pub static ref BLOCK_TIMESTAMP_TOLERANCE: Duration = Duration::hours(2);
}

pub type BlockNonce = [u8; 32];

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BlockHeader {
    pub version: u32,
    pub prev_hash: Hash256,
    pub merkle_root: Hash256,
    pub timestamp: DateTime<Utc>,
    pub difficulty_target: Hash256,
    pub nonce: BlockNonce,
    pub hash: Hash256,
}

/// Everything except the hash, so that this block can be hashed
#[derive(Serialize, Deserialize)]
pub struct RawBlockHeader {
    pub version: u32,
    pub prev_hash: Hash256,
    pub merkle_root: Hash256,
    pub timestamp: DateTime<Utc>,
    pub difficulty_target: Hash256,
    pub nonce: BlockNonce,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BlockchainDB {
    pub blocks: Vec<Block>,
    pub forks: Vec<ForkChain>,
    pub orphans: Vec<Block>,
    pub utxo_pool: UTXOPool
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ForkChain {
    /// The index of the previous block in the MAIN chain.
    pub prev_index: usize,
    pub blocks: Vec<Block>
}

impl From<&BlockHeader> for RawBlockHeader {
    fn from(block: &BlockHeader) -> Self {
        Self {
            version: block.version,
            prev_hash: block.prev_hash,
            merkle_root: block.merkle_root,
            timestamp: block.timestamp,
            difficulty_target: block.difficulty_target,
            nonce: block.nonce
        }
    }
}

impl std::fmt::Debug for BlockHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockHeader")
            .field("version", &self.version)
            .field("prev_hash", &hex::encode(&self.prev_hash))
            .field("merkle_root", &hex::encode(&self.merkle_root))
            .field("timestamp", &self.timestamp)
            .field("difficulty_target", &hex::encode(&self.difficulty_target))
            .field("nonce", &hex::encode(&self.nonce))
            .field("hash", &hex::encode(&self.hash))
            .finish()
    }
}

impl Block {
    pub fn get_txn(&self, hash: Hash256) -> Option<Transaction> {
        self.transactions.iter().find(|t| t.hash == hash).cloned()
    }

    pub fn size(&self) -> usize {
        self.header.size() +
        self.transactions
            .iter()
            .fold(0, |a, e| a + e.size())
    }

    /// Gets all transactions in the block, consuming the block in the
    /// process.
    pub fn to_txns(self) -> Vec<Transaction> {
        return self.transactions;
    }


    pub fn get_merkle_root(&self) -> Option<Hash256> {
        let txns: Vec<[u8; 32]> = self.transactions.iter().
        map(|t| t.hash)
        .collect();

        while txns.len() > 1 {
            // An array to store intermediate values when building the merkle tree.
            let mut array:Vec<[u8; 32]> ;

            // Iterate through transactions two at a time.
            for i in (0..txns.len()).step_by(2) {
                let hash: [u8; 32];

                // Combine the hash of two transactions.
                for j in 0..txns[i].len() {
                    hash[i] = txns[i][j] + txns[i + 1][j];
                }

                // Push the hash onto the intermediate array.
                // The size of the array should decrease by 1/2 with every loop.
                array.push(hash)
            }
            txns = array
        }
        Some(txns[0])
    }

}

impl BlockHeader {
    pub fn size(&self) -> usize {
        size_of_val(&self.version) +
        size_of_val(&self.prev_hash) +
        size_of_val(&self.merkle_root) +
        size_of_val(&self.timestamp) +
        size_of_val(&self.difficulty_target) +
        size_of_val(&self.nonce) +
        size_of_val(&self.hash)
    }
}

impl BlockchainDB {
    /// Returns the size of the best chain (the "best height"), the index of the best chain, and whether or not
    /// the best chain is not uniquely the best (i.e., true if there is another equally valid chain).
    /// The index will be 0 if the best chain is the main chain and 1,2,3...n for the nth fork.
    pub fn best_chain(&self) -> (usize, usize, bool) {
        // We can assume that fork chains are always sorted by index, so that the earliest fork chain is first.
        if self.forks.len() == 0 {
            return (self.blocks.len(), 0, false);
        }

        let start_i = self.forks[0].prev_index;

        // The total difficulty targets from the point of the earliest fork to the last block on the
        // main chain
        let main_diff = self.blocks[start_i..].iter().fold(BigUint::zero(), |a, e| a + BigUint::from_bytes_be(&e.header.difficulty_target));
        let fork_diffs = 
            self.forks
                .iter()
                .map(|f| {
                    // Add up the difficulties between the earliest fork and the current fork (on the main chain)
                    self.blocks[start_i..f.prev_index]
                        .iter()
                        .fold(BigUint::zero(), |a, e| a + BigUint::from_bytes_be(&e.header.difficulty_target))
                    +

                    // Add up the difficulties on the current fork
                    f.blocks[0..]
                        .iter()
                        .fold(BigUint::zero(), |a, e| a + BigUint::from_bytes_be(&e.header.difficulty_target))
                })
                .collect::<Vec<BigUint>>();

        // A higher difficulty target corresponds to an easier difficulty, so what we actually want after summing
        // up difficulty targets is their minimum.
        let min_fork_diff = fork_diffs.iter().min().unwrap();
        let min_index = fork_diffs.iter().position(|f| f == min_fork_diff).unwrap();

        // After computing the best fork difficulty, check if the main chain is still more difficult
        if main_diff < *min_fork_diff {
            (self.blocks.len(), 0, false);
        }

        // Check if the main chain has the same difficulty as a fork and that this is the best difficulty
        if fork_diffs.contains(&main_diff) && main_diff == *min_fork_diff {
            // Select the main chain if we have a fork with duplicate difficulty
            return (self.blocks.len(), 0, true);
        }

        // There may be two forks with duplicate validity but we don't care because this is rare
        (self.forks[min_index].blocks.len() + self.forks[min_index].prev_index, min_index + 1, false)
    }

    pub fn get_chain<'a>(&'a self, index: usize) -> &'a Vec<Block> {
        if index == 0 {
            return &self.blocks;
        }

        return &self.forks[index].blocks;
    }

    pub fn top_hash(&self, chain_idx: usize) -> Hash256 {
        self.get_chain(chain_idx).last().unwrap().header.hash
    }

    /// Returns the block, the chain index, and the block's position in the chain.
    /// Returns none if the block does not exist anywhere in the blockchain.
    /// Searches the blockchain in reverse because we're usually going to be looking for recent blocks.
    pub fn get_block<'a>(&'a self, hash: Hash256) -> Option<(&'a Block, usize, usize)> {
        for i in (0..self.blocks.len()).rev() {
            let block = &self.blocks[i];
            if block.header.hash == hash {
                return Some((block, 0, i));
            }
        }

        for i in 0..self.forks.len() {
            let blocks = &self.forks[i].blocks;

            for j in (0..blocks.len()).rev() {
                let block = &blocks[j];
                if block.header.hash == hash {
                    return Some((block, i + 1, j));
                }
            }
        }

        None
    }

    pub fn get_block_mut<'a>(&'a mut self, hash: Hash256) -> Option<(&'a Block, usize, usize)> {
        for i in (0..self.blocks.len()).rev() {
            let block = &self.blocks[i];
            if block.header.hash == hash {
                return Some((block, 0, i));
            }
        }

        for i in 0..self.forks.len() {
            let blocks = &self.forks[i].blocks;

            for j in (0..blocks.len()).rev() {
                let block = &blocks[j];
                if block.header.hash == hash {
                    return Some((block, i + 1, j));
                }
            }
        }

        None
    }

    /// Returns the blocks from the starting position to the end position. It is the caller's
    /// job to ensure that a valid chain index is passed in as well as valid `start_pos` and `end_pos`.
    /// 
    /// If the chain index is nonzero, positions are still interpreted as starting from the genesis
    /// block. The blocks returned may consist of some in the main chain and some in the fork.
    /// The `end_pos` here is the absolute position from the genesis block, NOT the position in the chain.
    /// See [get_blocks_rel<'a>()] for a version of this method that interprets `end_pos` as a relative position
    /// indicating the block's height in the fork.
    pub fn get_blocks(&self, chain: usize, start_pos: usize, end_pos: usize) -> Vec<Block> {
        if chain == 0 {
            return self.blocks[start_pos..end_pos].to_vec();
        }

        let chain = &self.forks[chain];
        let start_pos_offset = start_pos as isize - chain.prev_index as isize;
        let end_pos_offset = end_pos as isize - chain.prev_index as isize;

        let mut out: Vec<Block> = vec![];

        if start_pos_offset < 0 && end_pos_offset < 0 {
            let main_blocks = self.blocks[start_pos..end_pos].to_vec();
            
            for block in main_blocks {
                out.push(block);
            }
        } else if start_pos_offset < 0 {
            let main_blocks = self.blocks[start_pos..chain.prev_index].to_vec();
            
            for block in main_blocks {
                out.push(block);
            }
        }

        let fork_blocks = chain.blocks[0..(end_pos_offset as usize)].to_vec();

        for block in fork_blocks {
            out.push(block);
        }

        out
    }

    /// This is used to rebuild the entire UTXO database when verifying new blocks, which is a waste of space
    /// and memory. If we're going to be rebuilding the database it would be more prudent to pass around indices into
    /// the blockchain, instead of copies of it.
    pub fn get_blocks_rel(&self, chain: usize, start_pos: usize, end_pos: usize) -> Vec<Block> {
        let mut out: Vec<Block> = vec![];

        if chain == 0 {
            for block in &self.blocks[start_pos..end_pos] {
                out.push(block.clone());
            }

            return out;
        }

        let chain = &self.forks[chain];

        for block in &self.blocks[start_pos..(chain.prev_index + 1)] {
            out.push(block.clone());
        }

        for block in &chain.blocks[0..end_pos] {
            out.push(block.clone());
        }

        out
    }

    /// Finds the given transaction in the entire blockchain. Returns the block containing the
    /// transaction, the chain index of the block, and the transaction if found.
    pub fn find_txn<'a>(&'a self, hash: Hash256) -> Option<(&'a Block, usize, Transaction)> {
        for i in 0..self.blocks.len() {
            let block = &self.blocks[i];
            let txn_opt = block.get_txn(hash);

            if txn_opt.is_some() {
                return Some((block, 0, txn_opt.unwrap()));
            }
        }

        for chain_idx in 0..self.forks.len() {
            let fork_blocks = &self.forks[chain_idx].blocks;

            for i in 0..fork_blocks.len() {
                let block = &fork_blocks[i];
                let txn_opt = block.get_txn(hash);

                if txn_opt.is_some() {
                    return Some((block, chain_idx, txn_opt.unwrap()));
                }
            }
        }

        None
    }

    pub fn current_difficulty(&self) -> Hash256 {
        self.blocks.last().unwrap().header.difficulty_target
    }

    pub fn add_block(&mut self, block: Block) {
        let (_, chain, pos) = self.get_block(block.header.prev_hash).unwrap();
        let top = match chain {
            0 => self.blocks.last().unwrap(),
            i => self.forks[i - 1].blocks.last().unwrap()
        };

        // Best condition, we don't need to create a new fork
        if top.header.hash == block.header.prev_hash {
            match chain {
                0 => self.blocks.push(block),
                i => self.forks[i - 1].blocks.push(block)
            };
            return;
        }

        // TODO: Support this?
        // It's so rare that it might not even be worth supporting - it's a lot
        // of extra logic. If it does happen it will definitely cause weird bugs
        // but it might not be worth it given how rarely such a bug would occur.
        if chain != 0 {
            println!("We have encoutered a fork of a fork");
            return;
        }

        self.forks.push(ForkChain {
            prev_index: pos,
            blocks: vec![block],
        });
    }

    fn resolve_forks(&mut self) -> Vec<Block> {
        if self.forks.len() == 0 {
            return vec![];
        }

        // First figure out the best chain
        let (_, chain_idx, is_dup) = self.best_chain();

        // We can't resolve forks if we have two equally valid chains
        if is_dup {
            return vec![];
        }

        let mut out: Vec<Block> = vec![];

        // If the best chain is the main one, then just delete the forks. We need
        // to keep the blocks so that the transactions within them can be added to the pending pool
        if chain_idx == 0 {
            for fork in &self.forks {
                for block in &fork.blocks {
                    out.push(block.clone());
                }
            }
        } else {
            let winning_fork = &self.forks[chain_idx - 1];

            // Remove the extra blocks on the main chain
            for i in (winning_fork.prev_index + 1)..self.blocks.len() {
                out.push(self.blocks.remove(i));
            }

            // Remove the blocks in other forks
            for i in (0..self.forks.len()).filter(|i| *i != (chain_idx - 1)) {
                let fork = &self.forks[i];

                for block in &fork.blocks {
                    out.push(block.clone());
                }
            }

            // Move the fork blocks to the main chain
            let new_top_blocks = &winning_fork.blocks;
            for block in new_top_blocks {
                self.blocks.push(block.clone());
            }
        }

        self.forks = vec![];

        out
    }
}

pub fn check_orphans(state: &mut State) {
    let mut orphans_to_remove: Vec<usize> = vec![];

    for i in 0..state.blockchain.orphans.len() {
        let block = &state.blockchain.orphans[i];
        let verify_result = verify_block(block.clone(), state);

        match verify_result {
            // Block is no longer an orphan!
            Ok(false) => {
                orphans_to_remove.push(i);
            },
            Err(err) => {
                println!("Error verifying orphan block: {}", err.to_string());
                orphans_to_remove.push(i);
            },
            // Block is still an orphan
            Ok(true) => ()
        };
    }

    for pos in orphans_to_remove {
        state.blockchain.orphans.remove(pos);
    }
}

/// Tries to resolve any forks in the blockchain. If there is a unique best chain,
/// this function will get any blocks that need to be removed from the blockchain,
/// and it will add their transactions back to the pending/orphan pools as well as
/// update the UTXO database accordingly.
pub fn resolve_forks(state: &mut State) {
    let mut fork_blocks = state.blockchain.resolve_forks();

    if fork_blocks.len() == 0 {
        return;
    }

    let mut txns: Vec<Transaction> = vec![];

    for block in fork_blocks.drain(0..) {
        txns.append(&mut block.to_txns());
    }

    state.pending_txns.append(&mut txns);

    // Reset the UTXO database, then check all pending and orphan transactions.
    // We need to maintain the invariant that every pending or orphan transaction is valid
    // and is accounted for by the UTXO pool.
    state.blockchain.utxo_pool = build_utxos_from_confirmed(&state.blockchain.blocks);
    check_pending_and_orphans(state);
}

pub fn genesis_block() -> Block {
    let genesis_miner = b58c_to_address(String::from("2LuJkN1xDRRM2R2h2H4qnSspy4qmwoZfor")).expect("Failed to create genesis block");
    let coinbase = make_coinbase_txn(&genesis_miner, String::from("genesis block"), 0);
    let target_bytes = hex::decode("00000000f0000000000000000000000000000000000000000000000000000000").unwrap();
    let mut target = [0 as u8; 32];
    target.copy_from_slice(&target_bytes);

    // TODO: Make Merkle root correctly

    let mut header = BlockHeader {
        version: 1,
        prev_hash: [0; 32],
        merkle_root: coinbase.hash,
        timestamp: Utc.ymd(2022, 11, 4).and_hms(12, 0, 0),
        difficulty_target: target,
        nonce: [0x45; 32],
        hash: [0; 32]
    };

    let raw: RawBlockHeader = (&header).into();
    let raw_bytes = bincode::serialize(&raw).expect("Failed to serialize genesis block header");
    let mut context = Context::new(&SHA256);
    context.update(&raw_bytes);
    let digest = context.finish();
    let hash = digest.as_ref();

    header.hash.copy_from_slice(hash);

    Block {
        header,
        transactions: vec![coinbase],
    }
}

pub fn hash_block_header(header: &RawBlockHeader) -> Hash256 {
    let bytes = bincode::serialize(header).unwrap();
    let mut context = Context::new(&SHA256);
    context.update(&bytes);
    let digest = context.finish();
    let hash = digest.as_ref();

    let mut out = [0 as u8; 32];
    out.copy_from_slice(hash);

    out
}
