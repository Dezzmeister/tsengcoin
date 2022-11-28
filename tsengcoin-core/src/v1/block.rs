use chrono::{DateTime, Utc, TimeZone};
use num_bigint::BigUint;
use num_traits::Zero;
use ring::digest::{Context, SHA256};
use serde::{Serialize, Deserialize};

use crate::{wallet::{Hash256, b58c_to_address}};

use super::transaction::{Transaction, make_coinbase_txn, UTXOPool};

/// Max size of a block in bytes
pub const MAX_BLOCK_SIZE: usize = 16384;

pub type BlockNonce = [u8; 32];

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    pub difficulty_bits: u32,
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
    pub fn get_block<'a>(&'a self, hash: Hash256) -> Option<(&'a Block, usize, usize)> {
        for i in 0..self.blocks.len() {
            let block = &self.blocks[i];
            if block.header.hash == hash {
                return Some((block, 0, i));
            }
        }

        for i in 0..self.forks.len() {
            let blocks = &self.forks[i].blocks;

            for j in 0..blocks.len() {
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
}

pub fn genesis_block() -> Block {
    let genesis_miner = b58c_to_address(String::from("2LuJkN1xDRRM2R2h2H4qnSspy4qmwoZfor")).expect("Failed to create genesis block");
    let coinbase = make_coinbase_txn(&genesis_miner, String::from("genesis block"), 0);
    let difficulty_bits: u32 = 0x1cf0_0000;
    // The difficulty "1cf00000" will produce the target hash here. You can verify this by running `get-target`.
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
        difficulty_bits
    }
}
