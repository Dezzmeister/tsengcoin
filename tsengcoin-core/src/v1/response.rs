use std::{
    error::Error,
    net::{SocketAddr, TcpStream},
    sync::{
        Arc, Mutex,
    },
};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    v1::{
        chain_request::{check_pending_dh, make_dh_response_req, make_intent_req},
        request::send_new_txn,
        transaction::get_p2pkh_sender,
    },
    wallet::Hash256, gui::bridge::is_connection_accepted,
};

use super::{
    block::Block,
    block_verify::verify_block,
    chain_request::{decompose_dh_req, is_dh_req, is_dh_req_to_me},
    encrypted_msg::{decompose_enc_req, handle_chain_request, is_enc_req, is_enc_req_to_me},
    net::{DistantNode, Node, PROTOCOL_VERSION, broadcast_async, find_new_friends},
    request::{AdvertiseReq, GetAddrReq, GetBlocksReq, Request},
    state::{State, GUIChannels},
    transaction::Transaction,
    txn_verify::verify_transaction,
};

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    GetAddr(GetAddrRes),
    GetBlocks(GetBlocksRes),
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
    Blocks(Vec<Block>),
}

pub fn handle_request(
    req: Request,
    socket: TcpStream,
    gui_channels: &GUIChannels,
    state_arc: &Arc<Mutex<State>>,
) -> Result<(), Box<dyn Error>> {
    match req {
        Request::GetAddr(data) => handle_get_addr(data, socket, state_arc),
        Request::Advertise(data) => handle_advertise(data, socket, state_arc),
        Request::GetBlocks(data) => handle_get_blocks(data, socket, state_arc),
        Request::NewTxn(data) => {
            handle_new_txn(data, socket, gui_channels, state_arc)
        }
        Request::NewBlock(data) => handle_new_block(data, socket, state_arc),
    }
}

fn handle_get_addr(
    data: GetAddrReq,
    socket: TcpStream,
    state_mut: &Mutex<State>,
) -> Result<(), Box<dyn Error>> {
    let peer_remote_addr = match socket.peer_addr() {
        Ok(addr) => addr.ip(),
        Err(err) => return Err(format!("Failed to get peer address: {}", err).into())
    };
    let peer_remote_port = data.listen_port;
    let addr_you = SocketAddr::new(peer_remote_addr, peer_remote_port);

    let mut guard = state_mut.lock().unwrap();
    let state = &mut *guard;

    let neighbors: Vec<Node> = state.network.peers.iter().map(|p| p.to_owned()).collect();

    let (best_height, chain_idx, _) = state.blockchain.best_chain();

    let res = Response::GetAddr(GetAddrRes {
        version: PROTOCOL_VERSION,
        addr_you,
        neighbors,
        best_height,
        best_hash: state.blockchain.top_hash(chain_idx),
    });

    let node = Node {
        version: data.version,
        addr: addr_you,
        last_send: Utc::now(),
        best_height: Some(data.best_height),
        best_hash: Some(data.best_hash),
    };

    // Add the node back as a peer
    if !state.network.peers.contains(&node) {
        state.network.peers.push(node);
    }

    if state.remote_addr_me.is_none() {
        state.remote_addr_me = Some(data.addr_you);
    }

    state.network.clean(state.remote_addr_me.unwrap());

    if let Err(err) = send_res(res, &socket) {
        println!("Error sending reply back to node: {}", err);
    }

    Ok(())
}

fn handle_advertise(
    data: AdvertiseReq,
    socket: TcpStream,
    state_mut: &Mutex<State>,
) -> Result<(), Box<dyn Error>> {
    drop(socket);

    let addr_you = data.addr_me;

    let mut guard = state_mut.lock().unwrap();
    let state = &mut *guard;

    let addr_me = state.remote_addr_me.unwrap();

    if state.network.has_known(addr_you) || (addr_you == addr_me) {
        return Ok(());
    }

    state
        .network
        .known_nodes
        .push(DistantNode { addr: addr_you });

    let peers = state.network.peer_addrs();
    drop(guard);

    let mut dead_nodes = broadcast_async(Request::Advertise(data), &peers);

    if !dead_nodes.is_empty() {
        let state = &mut state_mut.lock().unwrap();
        state.network.prune_dead_nodes(&mut dead_nodes);
    }

    if rand::random::<u8>() % 2 == 0 {
        find_new_friends(state_mut);
    }

    Ok(())
}

fn handle_get_blocks(
    data: GetBlocksReq,
    socket: TcpStream,
    state_mut: &Mutex<State>,
) -> Result<(), Box<dyn Error>> {
    let mut guard = state_mut.lock().unwrap();
    let state = &mut *guard;

    let my_hash_idx_opt = state.blockchain.get_block(data.my_hash);
    let (my_hash_chain, my_hash_pos) = match my_hash_idx_opt {
        None => {
            let res = Response::GetBlocks(GetBlocksRes::UnknownHash(data.my_hash));
            if let Err(err) = send_res(res, &socket) {
                println!("Error sending reply back to node: {}", err);
            }

            return Ok(());
        }
        Some((_, chain_idx, pos)) => (chain_idx, pos),
    };

    let your_hash_idx_opt = state.blockchain.get_block(data.your_hash);
    let (your_hash_chain, your_hash_pos) = match your_hash_idx_opt {
        None => {
            let res = Response::GetBlocks(GetBlocksRes::UnknownHash(data.your_hash));
            if let Err(err) = send_res(res, &socket) {
                println!("Error sending reply back to node: {}", err);
            }

            return Ok(());
        }
        Some((_, chain_idx, pos)) => (chain_idx, pos),
    };

    if my_hash_chain != your_hash_chain && my_hash_chain != 0 {
        if let Err(err) = send_res(Response::GetBlocks(GetBlocksRes::DisconnectedChains), &socket) {
            println!("Error sending reply back to node: {}", err);
        }

        return Ok(());
    }

    if your_hash_chain != 0 && (your_hash_chain - 1) > state.blockchain.forks.len() {
        if let Err(err) = send_res(Response::GetBlocks(GetBlocksRes::BadChainIndex), &socket) {
            println!("Error sending reply back to node: {}", err);
        }

        return Ok(());
    }

    if your_hash_pos <= my_hash_pos {
        if let Err(err) = send_res(Response::GetBlocks(GetBlocksRes::BadHashes), &socket) {
            println!("Error sending reply back to node: {}", err);
        }

        return Ok(());
    }

    let blocks = state
        .blockchain
        .get_blocks(my_hash_chain, my_hash_pos + 1, your_hash_pos + 1);

    if let Err(err) = send_res(Response::GetBlocks(GetBlocksRes::Blocks(blocks)), &socket) {
        println!("Error sending reply back to node: {}", err);
    }

    Ok(())
}

pub fn handle_new_txn(
    data: Transaction,
    socket: TcpStream,
    gui_channels: &GUIChannels,
    state_arc: &Arc<Mutex<State>>,
) -> Result<(), Box<dyn Error>> {
    drop(socket);

    let mut guard = state_arc.lock().unwrap();
    let state = &mut *guard;

    // Don't propagate transactions we already have
    if state.pending_txns.contains(&data) || state.orphan_txns.contains(&data) {
        return Ok(());
    }

    // The first thing we do is verify the transaction
    let verify_result = verify_transaction(data.clone(), state);

    let is_orphan = match verify_result {
        Err(_) => {
            return Ok(());
        }
        Ok(is_orphan) => is_orphan,
    };

    match is_orphan {
        true => state.orphan_txns.push(data.clone()),
        false => {
            state.add_pending_txn(data.clone());
        }
    };

    let peers = state.network.peer_addrs();
    drop(guard);

    let mut dead_nodes = broadcast_async(Request::NewTxn(data.clone()), &peers);

    let mut guard = state_arc.lock().unwrap();
    let state = &mut *guard;

    if !dead_nodes.is_empty() {
        state.network.prune_dead_nodes(&mut dead_nodes);
    }

    if is_enc_req(&data) && is_enc_req_to_me(&data, state) {
        let enc_req = decompose_enc_req(&data).unwrap();
        let sender = get_p2pkh_sender(&data, state).unwrap();
        let chain_req = match state.friends.decrypt_from_sender(enc_req, sender) {
            Ok(req) => req,
            Err(err) => {
                println!("Error decrypting chain request to us: {}", err);
                return Ok(());
            }
        };

        handle_chain_request(chain_req, sender, state, state_arc)?;
    }

    // Someone wants to chat with us; they initiated a Diffie-Hellman key exchange with
    // us and we can choose to respond
    if is_dh_req(&data) && is_dh_req_to_me(&data, state) {
        // TODO: Banned address list
        let sender_pubkey = decompose_dh_req(&data).unwrap();
        let sender = get_p2pkh_sender(&data, state).unwrap();
        let sender_name = state.friends.get_name(sender);

        let should_respond_dh = match check_pending_dh(sender_pubkey, sender, state) {
            Ok(val) => val,
            Err(err) => {
                return Err(format!("Error handling Diffie-Hellman request: {}", err).into());
            }
        };

        if !should_respond_dh {
            // We now need to make a transaction with an encrypted request
            println!("Completed Diffie-Hellman key exchange with {}", sender_name);

            match make_intent_req(sender, state)? {
                Some(intent_req) => {
                    send_new_txn(intent_req, state)?;
                    println!("Sending encrypted request");
                }
                None => {
                    println!("You can now send chain requests to {}", sender_name);
                }
            };

            return Ok(());
        }

        let has_gui = state.has_gui();
        let default = state.friends.fallback_accept_connections;

        // Release the mutex while we wait for a response from the main thread so that we don't hold
        // up the rest of the program
        drop(guard);

        let accept_request = is_connection_accepted(
            sender_name.clone(),
            gui_channels,
            has_gui,
            default
        )?;

        if !accept_request {
            println!("Rejected connection request by {}", sender_name);
            return Ok(());
        }

        println!(
            "Accepted connection request from {}. Finishing Diffie-Hellman exchange",
            sender_name
        );

        let mut guard = state_arc.lock().unwrap();
        let state = &mut *guard;
        let (response_req, _) = make_dh_response_req(&data, state)?;
        send_new_txn(response_req, state)?;
    }

    Ok(())
}

pub fn handle_new_block(
    data: Block,
    socket: TcpStream,
    state_mut: &Mutex<State>,
) -> Result<(), Box<dyn Error>> {
    drop(socket);

    let mut guard = state_mut.lock().unwrap();
    let state = &mut *guard;

    let block_hash = data.header.hash;

    let existing_block = state.blockchain.get_block(block_hash);
    if existing_block.is_some() {
        return Ok(());
    }

    let verify_result = verify_block(data.clone(), state);

    match verify_result {
        Err(err) => {
            println!("Error verifying block: {}", err);
            return Ok(());
        }
        Ok(true) => {
            println!("Received new orphan block: {}", hex::encode(&block_hash));
            return Ok(());
        }
        Ok(false) => {
            println!("Received new block: {}", hex::encode(&block_hash));
        }
    };

    state.resolve_forks();

    let peers = state.network.peer_addrs();
    drop(guard);

    let mut dead_nodes = broadcast_async(Request::NewBlock(data), &peers);

    if !peers.is_empty() {
        let state = &mut state_mut.lock().unwrap();
        state.network.prune_dead_nodes(&mut dead_nodes);
    }

    Ok(())
}

pub fn send_res(res: Response, stream: &TcpStream) -> bincode::Result<()> {
    bincode::serialize_into(stream, &res)
}
