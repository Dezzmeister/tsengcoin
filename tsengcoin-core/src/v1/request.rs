use std::{
    error::Error,
    net::{SocketAddr, TcpStream},
    cmp::min,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    v1::{block_verify::verify_block, net::DistantNode},
    wallet::Hash256,
};

use super::{
    block::Block,
    net::{Node, PROTOCOL_VERSION, MAX_NEIGHBORS, broadcast_async_blast},
    response::{
        GetBlocksRes::{BadChainIndex, BadHashes, Blocks, DisconnectedChains, UnknownHash},
        Response,
    },
    state::State,
    transaction::Transaction,
};

const MAX_UNKNOWN_HASH_ATTEMPTS: usize = 3;

#[derive(Serialize, Deserialize, Debug)]
pub enum Request {
    GetAddr(GetAddrReq),
    Advertise(AdvertiseReq),
    GetBlocks(GetBlocksReq),
    NewTxn(Transaction),
    NewBlock(Block),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetAddrReq {
    pub version: u32,
    pub addr_you: SocketAddr,
    pub listen_port: u16,
    pub best_height: usize,
    pub best_hash: Hash256,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AdvertiseReq {
    pub addr_me: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetBlocksReq {
    pub your_hash: Hash256,
    pub my_hash: Hash256,
}

pub fn get_first_peers(
    known_node: SocketAddr,
    state: &mut State,
) -> Result<(), Box<dyn Error>> {
    let (best_height, chain_idx, _) = state.blockchain.best_chain();

    let req = Request::GetAddr(GetAddrReq {
        version: PROTOCOL_VERSION,
        addr_you: known_node,
        listen_port: state.local_addr_me.port(),
        best_height,
        best_hash: state.blockchain.top_hash(chain_idx),
    });

    let res = send_req(&req, &known_node)?;

    match res {
        Response::GetAddr(data) => {
            for node in data.neighbors {
                if node == data.addr_you {
                    continue;
                }

                state.network.peers.push(node);
            }

            state.network.peers.push(Node {
                version: data.version,
                addr: known_node,
                last_send: Utc::now(),
                best_height: Some(data.best_height),
                best_hash: Some(data.best_hash),
            });

            state
                .network
                .known_nodes
                .push(DistantNode { addr: known_node });

            // TODO: Bootstrap with a few nodes to reduce the chances of a node lying about your remote IP
            state.remote_addr_me = Some(data.addr_you);
            state.network.clean(data.addr_you);

            Ok(())
        }
        _ => Err("Known node responded with nonsense".into()),
    }
}

pub fn discover(seed_addr: SocketAddr, state: &mut State) -> Result<(), Box<dyn Error>> {
    let addrs: Vec<SocketAddr> = state
        .network
        .peers
        .iter()
        .filter(|n| *n != seed_addr)
        .map(|n| n.addr)
        .collect();

    let (best_height, chain_idx, _) = state.blockchain.best_chain();

    for addr in addrs {
        let req = Request::GetAddr(GetAddrReq {
            version: PROTOCOL_VERSION,
            addr_you: addr,
            listen_port: state.local_addr_me.port(),
            best_height,
            best_hash: state.blockchain.top_hash(chain_idx),
        });

        let result = send_req(&req, &addr);

        match result {
            Err(_) => state.network.remove(addr),
            Ok(Response::GetAddr(mut data)) => {
                state.network.peers.append(&mut data.neighbors);

                for mut peer in &mut state.network.peers {
                    if peer == &addr {
                        peer.best_height = Some(data.best_height);
                        peer.best_hash = Some(data.best_hash);
                    }
                }
            }
            // Remove nodes that return nonsense
            _ => state.network.remove(addr),
        }
    }

    for node in &state.network.peers {
        state.network.known_nodes.push(node.into());
    }

    let addr_me = state.remote_addr_me.unwrap();

    state.network.merge(addr_me);
    state.network.clean(addr_me);
    state.network.shuffle();
    let num_peers = min(state.network.peers.len(), MAX_NEIGHBORS);
    state.network.peers = state.network.peers[0..num_peers].to_vec();

    Ok(())
}

pub fn download_latest_blocks(state: &mut State) -> Result<(), Box<dyn Error>> {
    let best_node_opt = state.network.most_updated_node();
    let best_node = match best_node_opt {
        None => {
            return Err("No suitable nodes to update local blockchain".into());
        }
        Some(node) => node,
    };

    if best_node.best_height.unwrap() == 1 {
        return Ok(());
    }

    let mut block_idx = state.blockchain.blocks.len();
    let mut hash = state.blockchain.blocks[block_idx - 1].header.hash;

    // Now we ask the other node to send us some blocks. If we disconnected during a fork, we may have some
    // blocks that are no longer on the blockchain. We will know this if we get an `UnknownHash` response,
    // so we should try again a few different times with the previous hash.

    let mut attempt: usize = 0;

    while attempt < MAX_UNKNOWN_HASH_ATTEMPTS && block_idx > 0 {
        let req = Request::GetBlocks(GetBlocksReq {
            your_hash: best_node.best_hash.unwrap(),
            my_hash: hash,
        });

        let res = send_req(&req, &best_node.addr)?;

        match res {
            Response::GetBlocks(res_data) => {
                match res_data {
                    Blocks(blocks) => {
                        if blocks[0].header.prev_hash == hash {
                            for block in blocks {
                                let verify_result = verify_block(block.clone(), state);

                                match verify_result {
                                    Ok(false) => (),
                                    Err(err) => {
                                        println!("Received a bad block: {}", err);
                                    }
                                    Ok(true) => {
                                        println!("Received an orphan block as part of a blockchain from another peer");
                                        // TODO: Remove peer for this nonsense. This really is nonsense because we checked earlier that
                                        // this chain of blocks is connected to the top of our main chain, so it would be the peer's
                                        // fault for inserting a disconnected block into the blocks it sends back.
                                    }
                                }
                            }
                        } else {
                            return Err("Received block with bad prev hash".into());
                        }
                        break;
                    }
                    UnknownHash(_) => {
                        block_idx -= 1;
                        hash = state.blockchain.blocks[block_idx - 1].header.hash;

                        println!("Received `UnknownHash` while trying to download blockchain");
                    }
                    DisconnectedChains => {
                        return Err("Tried to download blockchain across unconnected forks".into())
                    }
                    BadChainIndex => {
                        return Err("Tried to download blockchain with bad chain index".into())
                    }
                    BadHashes => return Err("Tried to download blockchain with bad hashes".into()),
                }
            }
            _ => {
                // TODO: Remove node and try again
                return Err("Peer node returned nonsense".into());
            }
        }

        attempt += 1;
    }

    println!("Up to date: {} blocks", state.blockchain.blocks.len());

    Ok(())
}

pub fn advertise_self(state: &mut State) -> Result<(), Box<dyn Error>> {
    let addr_me = state.remote_addr_me.unwrap();

    let req = Request::Advertise(AdvertiseReq { addr_me });

    let peers = state.network.peer_addrs();
    broadcast_async_blast(req, &peers, None);

    Ok(())
}

/// Broadcast a new transaction to the network. Assumes the transaction is valid - it is
/// the caller's job to check this beforehand.
pub fn send_new_txn(txn: Transaction, state: &mut State) -> Result<(), Box<dyn Error>> {
    // TODO: Pay attention to these errors
    let peers = state.network.peer_addrs();
    broadcast_async_blast(Request::NewTxn(txn), &peers, None);

    Ok(())
}

pub fn send_req(req: &Request, addr: &SocketAddr) -> bincode::Result<Response> {
    let socket = TcpStream::connect(addr)?;
    socket.set_nodelay(true).unwrap();
    bincode::serialize_into(&socket, &req)?;

    let res: Response = bincode::deserialize_from(&socket)?;

    Ok(res)
}

pub fn send_msg(msg: &Request, addr: &SocketAddr) -> bincode::Result<()> {
    let socket = TcpStream::connect(addr)?;
    socket.set_nodelay(true).unwrap();
    bincode::serialize_into(&socket, &msg)?;

    Ok(())
}
