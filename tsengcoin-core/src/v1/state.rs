use std::{
    collections::HashMap,
    error::Error,
    fs,
    net::SocketAddr,
    sync::mpsc::{channel, Receiver, Sender},
};

use ring::signature::{EcdsaKeyPair, KeyPair};

#[cfg(feature = "gui")]
use crate::gui::gui::{GUIRequest, GUIResponse, GUIState};

use crate::{
    wallet::{address_from_public_key, Address, Hash256},
};

use super::{
    block::{genesis_block, resolve_forks, Block, BlockchainDB},
    chain_request::FriendState,
    miners::{api::MinerMessage, stats::MinerStatsState},
    net::Network,
    transaction::{Transaction, TransactionIndex, UTXOPool},
};

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
    /// A "friend" is someone who has completed a Diffie-Hellman key exchange with us. Friends can send each other encrypted requests using
    /// a shared secret.
    /// TODO: Double ratchet
    pub friends: FriendState,
    #[cfg(feature = "gui")]
    pub gui_req_sender: Sender<GUIRequest>,
    #[cfg(feature = "gui")]
    pub gui: Option<GUIState>,
    pub miner: Option<String>,
    pub miner_stats: Option<MinerStatsState>,
    /// Work group size, only meaningful for the CL miner.
    pub wg_size: Option<usize>,
    /// Number of work groups
    pub num_work_groups: Option<usize>,

    miner_channel: Sender<MinerMessage>,
}

#[cfg(feature = "gui")]
pub struct GUIChannels {
    pub req_channel: Sender<GUIRequest>,
    pub res_channel: Receiver<GUIResponse>
}

#[cfg(not(feature = "gui"))]
pub struct GUIChannels {}

impl State {
    pub fn new(
        addr_me: SocketAddr,
        keypair: EcdsaKeyPair,
        #[cfg(feature = "gui")]
        gui_req_sender: Sender<GUIRequest>,
        #[cfg(feature = "gui")]
        gui: Option<GUIState>,
        miner: Option<String>,
    ) -> (Self, Receiver<MinerMessage>) {
        let address = address_from_public_key(&keypair.public_key().as_ref().to_vec());
        let blockchain = load_blockchain_db();
        let (miner_sender, miner_receiver) = channel();

        (
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
                orphan_txns: vec![],
                hashes_per_second: 0,
                friends: FriendState {
                    pending_dh: HashMap::new(),
                    intents: HashMap::new(),
                    aliases: HashMap::new(),
                    keys: HashMap::new(),
                    exclusivity: 1,
                    chain_req_amount: 1,
                    chat_sessions: HashMap::new(),
                    fallback_accept_connections: false,
                },
                #[cfg(feature = "gui")]
                gui_req_sender,
                #[cfg(feature = "gui")]
                gui,
                miner,
                miner_stats: None,
                wg_size: None,
                num_work_groups: None,
                miner_channel: miner_sender,
            },
            miner_receiver,
        )
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
    where
        Transaction: PartialEq<T>,
    {
        self.pending_txns.iter().find(|t| **t == txn).cloned()
    }

    pub fn get_orphan_txn<T: PartialEq>(&self, txn: T) -> Option<Transaction>
    where
        Transaction: PartialEq<T>,
    {
        self.orphan_txns.iter().find(|t| **t == txn).cloned()
    }

    pub fn get_pending_or_confirmed_txn(&self, txn: Hash256) -> Option<Transaction> {
        let pending = self.pending_txns.iter().find(|t| **t == txn);

        if let Some(pending_txn) = pending {
            return Some(pending_txn.clone());
        }

        let block_txn_opt = self.blockchain.find_txn(txn);

        if let Some(block_txn) = block_txn_opt {
            return Some(block_txn.txn);
        }

        None
    }

    pub fn set_pending_txns(&mut self, new_txns: Vec<Transaction>) {
        let num_new_txns = new_txns.len() - self.pending_txns.len();
        self.pending_txns = new_txns;
        match self
            .miner_channel
            .send(MinerMessage::NewTransactions(num_new_txns))
        {
            Ok(_) | Err(_) => (),
        };
    }

    pub fn add_pending_txn(&mut self, txn: Transaction) {
        self.pending_txns.push(txn.clone());
        self.blockchain.utxo_pool.update_unconfirmed(&txn);
        match self.miner_channel.send(MinerMessage::NewTransactions(1)) {
            Ok(_) | Err(_) => (),
        };
    }

    pub fn add_block(&mut self, block: Block) {
        let hash = block.header.hash;
        self.blockchain.add_block(block);
        match self.miner_channel.send(MinerMessage::NewBlock(hash, true)) {
            Ok(_) | Err(_) => (),
        };
    }

    pub fn resolve_forks(&mut self) {
        if resolve_forks(self) {
            let hash = self.blockchain.top_hash(0);
            match self.miner_channel.send(MinerMessage::NewBlock(hash, true)) {
                Ok(_) | Err(_) => (),
            };
        }
    }

    /// Returns true if there is a main GUI attached to the program: TsengCoin core can run in
    /// a (nearly) headless mode or in a graphical mode.
    #[cfg(feature = "gui")]
    pub fn has_gui(&self) -> bool {
        self.gui.is_some()
    }

    #[cfg(not(feature = "gui"))]
    pub fn has_gui(&self) -> bool {
        false
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
            utxos: vec![TransactionIndex {
                block: Some(block_hash),
                txn: txn_hash,
                outputs: vec![0],
            }],
        },
    }
}
