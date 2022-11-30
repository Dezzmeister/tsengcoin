use std::{net::SocketAddr, fs, error::Error};

use ring::signature::{EcdsaKeyPair, KeyPair};

use crate::wallet::{Address, address_from_public_key, Hash256};

use super::{net::Network, block::{BlockchainDB, genesis_block}, transaction::{Transaction, UTXOPool, TransactionIndex}};

pub const DATA_DIR: &str = ".data";
pub const BLOCKCHAIN_DB_FILE: &str = "blockchain";

#[derive(Debug)]
pub struct State {
    pub local_addr_me: SocketAddr,
    pub remote_addr_me: Option<SocketAddr>,
    pub network: Network,
    pub keypair: EcdsaKeyPair,
    pub address: Address,
    pub blockchain: BlockchainDB,
    pub pending_txns: Vec<Transaction>,
    /// Valid transactions that reference a parent that does not exist.
    pub orphan_txns: Vec<Transaction>,
}

impl State {
    pub fn new(addr_me: SocketAddr, keypair: EcdsaKeyPair) -> Self {
        let address = address_from_public_key(&keypair.public_key().as_ref().to_vec());
        let blockchain = load_blockchain_db();

        Self {
            local_addr_me: addr_me,
            remote_addr_me: None,
            network: Network {
                peers: vec![],
                known_nodes: vec![],
            },
            keypair,
            address,
            blockchain,
            pending_txns: vec![],
            orphan_txns: vec![]
        }
    }

    /// TODO: Save the blockchain to a file
    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let db_bytes = bincode::serialize(&self.blockchain)?;

        fs::write(format!("{DATA_DIR}/{BLOCKCHAIN_DB_FILE}"), db_bytes)?;

        Ok(())
    }

    pub fn port(&self) -> u16 {
        self.local_addr_me.port()
    }

    pub fn get_pending_txn<T: PartialEq>(&self, txn: T) -> Option<Transaction>
        where Transaction: PartialEq<T>
    {
        self.pending_txns.iter().find(|t| **t == txn).cloned()
    }

    pub fn get_pending_or_confirmed_txn(&self, txn: Hash256) -> Option<Transaction> {
        let pending = self.pending_txns.iter().find(|t| **t == txn);
        if pending.is_some() {
            return Some(pending.unwrap().clone());
        }

        let block_txn_opt = self.blockchain.find_txn(txn);
        if block_txn_opt.is_some() {
            return Some(block_txn_opt.unwrap().2);
        }

        None
    }
}

pub fn load_blockchain_db() -> BlockchainDB {
    fs::create_dir_all(DATA_DIR).unwrap();

    let db_res = fs::read(format!("{DATA_DIR}/{BLOCKCHAIN_DB_FILE}"));
    if db_res.is_ok() {
        let bytes = db_res.unwrap();
        let out: BlockchainDB = bincode::deserialize(&bytes).unwrap();

        return out;
    }

    let genesis = genesis_block();
    let block_hash = genesis.header.hash;
    let txn_hash = genesis.transactions[0].hash;

    BlockchainDB {
        blocks: vec![genesis],
        forks: vec![],
        orphans: vec![],
        utxo_pool: UTXOPool {
            utxos: vec![
                TransactionIndex {
                    block: Some(block_hash),
                    txn: txn_hash,
                    outputs: vec![0]
                }
            ],
        },
    }
}
