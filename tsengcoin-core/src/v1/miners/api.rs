use std::sync::{mpsc::Receiver, Mutex};

use crate::{wallet::Hash256, v1::state::State};

#[cfg(feature = "cuda_miner")]
use super::cuda::mine;

pub enum MinerMessage {
    /// The argument here is the number of new transactions added to the pending pool
    NewTransactions(usize),
    /// The arguments are:
    ///     1. The hash of the newly added block
    ///     2. Whether the pending transaction pool has changed
    NewBlock(Hash256, bool),
    // The argument is the new difficulty target
    NewDifficulty(Hash256)
}

pub fn start_miner(state_mut: &Mutex<State>, miner_receiver: Receiver<MinerMessage>) {
    #[cfg(feature = "cuda_miner")]
    mine(state_mut, miner_receiver);
}
