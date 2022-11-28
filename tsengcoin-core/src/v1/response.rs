use std::{net::{TcpStream, SocketAddr}, error::Error, sync::{Mutex}};

use chrono::Utc;
use serde::{Serialize, Deserialize};

use crate::wallet::Hash256;

use super::{request::{Request, GetAddrReq, AdvertiseReq, GetBlocksReq}, state::{State}, net::{PROTOCOL_VERSION, Node, DistantNode}, block::Block, verify_error::VerifyResult, transaction::Transaction, verify::verify_transaction};

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    GetAddr(GetAddrRes),
    AdvertiseRes,
    GetBlocks(GetBlocksRes),
    NewTxn(VerifyResult<()>)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetAddrRes {
    pub version: u32,
    pub addr_you: SocketAddr,
    pub best_height: usize,
    pub best_hash: Hash256,
    pub neighbors: Vec<Node>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum GetBlocksRes {
    UnknownHash(Hash256),
    DisconnectedChains,
    BadChainIndex,
    BadHashes,
    Blocks(Vec<Block>)
}

pub fn handle_request(req: Request, socket: &TcpStream, state_mut: &Mutex<State>) -> Result<(), Box<dyn Error>> {
    match req {
        Request::GetAddr(data) => handle_get_addr(data, socket, state_mut),
        Request::Advertise(data) => handle_advertise(data, socket, state_mut),
        Request::GetBlocks(data) => handle_get_blocks(data, socket, state_mut),
        Request::NewTxn(data) => handle_new_txn(data, socket, state_mut)
    }
}

fn handle_get_addr(data: GetAddrReq, socket: &TcpStream, state_mut: &Mutex<State>) -> Result<(), Box<dyn Error>> {
    let peer_remote_addr = socket.peer_addr().unwrap().ip();
    let peer_remote_port = data.listen_port;
    let addr_you = SocketAddr::new(peer_remote_addr, peer_remote_port);

    let mut guard = state_mut.lock().unwrap();
    let state = &mut *guard;

    let neighbors: Vec<Node> = state.network.peers.iter().map(|p| p.to_owned()).collect();

    let (best_height, chain_idx, _)  = state.blockchain.best_chain();

    let res = Response::GetAddr(GetAddrRes {
        version: PROTOCOL_VERSION,
        addr_you,
        neighbors,
        best_height,
        best_hash: state.blockchain.top_hash(chain_idx)
    });

    let node = Node {
        version: data.version,
        addr: addr_you,
        last_send: Utc::now(),
        best_height: Some(data.best_height),
        best_hash: Some(data.best_hash)
    };

    // Add the node back as a peer
    if !state.network.peers.contains(&node) {
        state.network.peers.push(node);
    }

    if state.remote_addr_me.is_none() {
        state.remote_addr_me = Some(data.addr_you);
    }

    state.network.clean(state.remote_addr_me.unwrap());

    drop(guard);

    send_res(res, socket)?;

    Ok(())
}

fn handle_advertise(data: AdvertiseReq, socket: &TcpStream, state_mut: &Mutex<State>) -> Result<(), Box<dyn Error>> {
    let addr_you = data.addr_me;

    let mut guard = state_mut.lock().unwrap();
    let state = &mut *guard;

    send_res(Response::AdvertiseRes, socket)?;

    let addr_me = state.remote_addr_me.unwrap();

    if state.network.has_known(addr_you) || (addr_you == addr_me) {
        return Ok(());
    }

    state.network.known_nodes.push(DistantNode {
        addr: addr_you
    });

    state.network.broadcast(&Request::Advertise(data));

    if rand::random::<u8>() % 2 == 0 {
        let (best_height, chain_idx, _) = state.blockchain.best_chain();

        state.network.find_new_friends(state.port(), state.remote_addr_me.unwrap(), best_height, state.blockchain.top_hash(chain_idx));
    }

    Ok(())
}

fn handle_get_blocks(data: GetBlocksReq, socket: &TcpStream, state_mut: &Mutex<State>) -> Result<(), Box<dyn Error>> {
    let mut guard = state_mut.lock().unwrap();
    let state = &mut *guard;

    let my_hash_idx_opt = state.blockchain.get_block(data.my_hash);
    let (my_hash_chain, my_hash_pos) = match my_hash_idx_opt {
        None => {
            let res = Response::GetBlocks(GetBlocksRes::UnknownHash(data.my_hash));
            send_res(res, socket)?;

            return Ok(());
        },
        Some((_, chain_idx, pos)) => (chain_idx, pos)
    };

    let your_hash_idx_opt = state.blockchain.get_block(data.your_hash);
    let (your_hash_chain, your_hash_pos) = match your_hash_idx_opt {
        None => {
            let res = Response::GetBlocks(GetBlocksRes::UnknownHash(data.your_hash));
            send_res(res, socket)?;

            return Ok(());
        },
        Some((_, chain_idx, pos)) => (chain_idx, pos)
    };

    if my_hash_chain != your_hash_chain && my_hash_chain != 0 {
        send_res(Response::GetBlocks(GetBlocksRes::DisconnectedChains), &socket)?;

        return Ok(());
    }

    if your_hash_chain != 0 && (your_hash_chain - 1) > state.blockchain.forks.len() {
        send_res(Response::GetBlocks(GetBlocksRes::BadChainIndex), &socket)?;

        return Ok(());
    }

    if your_hash_pos <= my_hash_pos  {
        send_res(Response::GetBlocks(GetBlocksRes::BadHashes), &socket)?;

        return Ok(());
    }

    let blocks = state.blockchain.get_blocks(my_hash_chain, my_hash_pos + 1, your_hash_pos + 1);

    send_res(Response::GetBlocks(GetBlocksRes::Blocks(blocks)), &socket)?;

    Ok(())
}

pub fn handle_new_txn(data: Transaction, socket: &TcpStream, state_mut: &Mutex<State>) -> Result<(), Box<dyn Error>> {
    let mut guard = state_mut.lock().unwrap();
    let state = &mut *guard;

    // Don't propagate transactions we already have
    if state.pending_txns.contains(&data) || state.orphan_txns.contains(&data) {
        return Ok(());
    }

    let verify_result = verify_transaction(data.clone(), state_mut);

    let is_orphan = match verify_result {
        Err(err) => {
            send_res(Response::NewTxn(Err(err)), socket)?;
            return Ok(());
        },
        Ok(is_orphan) => is_orphan
    };

    match is_orphan {
        true => state.orphan_txns.push(data.clone()),
        false => {
            state.pending_txns.push(data.clone());
            state.blockchain.utxo_pool.update_unconfirmed(&data);
        }
    };

    state.network.broadcast(&Request::NewTxn(data));

    Ok(())
}

pub fn send_res(res: Response, stream: &TcpStream) -> bincode::Result<()> {
    bincode::serialize_into(stream, &res)
}