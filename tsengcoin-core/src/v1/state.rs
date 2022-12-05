use std::{net::SocketAddr, fs, error::Error, sync::mpsc::{Sender, Receiver, channel}, collections::HashMap};

use ring::signature::{EcdsaKeyPair, KeyPair};

use crate::{wallet::{Address, address_from_public_key, Hash256}};

use super::{net::Network, block::{BlockchainDB, genesis_block, Block, resolve_forks}, transaction::{Transaction, UTXOPool, TransactionIndex}, miners::api::MinerMessage, chat::ChatState};
use fltk::app::App;

/// TODO: Implement blockchain DB in filesystem or at least have a feature to enable it so we don't have to
/// download blocks every time
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
    pub hashes_per_second: usize,
    pub chat: ChatState,
    pub app: App,
    miner_channel: Sender<MinerMessage>
}

impl State {
    pub fn new(addr_me: SocketAddr, keypair: EcdsaKeyPair) -> (Self, Receiver<MinerMessage>) {
        let address = address_from_public_key(&keypair.public_key().as_ref().to_vec());
        let blockchain = load_blockchain_db();
        let (miner_sender, miner_receiver) = channel();
        
        let app = App::default();

        (Self {
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
            orphan_txns: vec![],
            hashes_per_second: 0,
            chat: ChatState {
                pending_dh: HashMap::new(),
                completed_dh: HashMap::new(),
                aliases: HashMap::new()
            },
            app,
            miner_channel: miner_sender,
        },
        miner_receiver)
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

    pub fn get_orphan_txn<T: PartialEq>(&self, txn: T) -> Option<Transaction>
        where Transaction: PartialEq<T>
    {
        self.orphan_txns.iter().find(|t| **t == txn).cloned()
    }

    pub fn get_pending_or_confirmed_txn(&self, txn: Hash256) -> Option<Transaction> {
        let pending = self.pending_txns.iter().find(|t| **t == txn);
        if pending.is_some() {
            return Some(pending.unwrap().clone());
        }

        let block_txn_opt = self.blockchain.find_txn(txn);
        if block_txn_opt.is_some() {
            return Some(block_txn_opt.unwrap().txn);
        }

        None
    }

    pub fn set_pending_txns(&mut self, new_txns: Vec<Transaction>) {
        let num_new_txns = new_txns.len() - self.pending_txns.len();
        self.pending_txns = new_txns;
        match self.miner_channel.send(MinerMessage::NewTransactions(num_new_txns)) {
            Ok(_) | Err(_) => ()
        };
    }

    pub fn add_pending_txn(&mut self, txn: Transaction) {
        self.pending_txns.push(txn.clone());
        self.blockchain.utxo_pool.update_unconfirmed(&txn);
        match self.miner_channel.send(MinerMessage::NewTransactions(1)) {
            Ok(_) | Err(_) => ()
        };
    }

    pub fn add_block(&mut self, block: Block) {
        let hash = block.header.hash;
        self.blockchain.add_block(block);
        match self.miner_channel.send(MinerMessage::NewBlock(hash, true)) {
            Ok(_) | Err(_) => ()
        };
    }

    pub fn resolve_forks(&mut self) {
        if resolve_forks(self) {
            let hash = self.blockchain.top_hash(0);
            match self.miner_channel.send(MinerMessage::NewBlock(hash, true)) {
                Ok(_) | Err(_) => ()
            };
        }
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
