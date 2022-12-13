use std::{
    collections::HashMap,
    sync::{mpsc::Receiver, Mutex},
};

use crate::{v1::state::State, wallet::Hash256};

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
