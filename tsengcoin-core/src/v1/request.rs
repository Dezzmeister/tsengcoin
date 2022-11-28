use std::{net::{SocketAddr, TcpStream}, error::Error, sync::{Mutex}};

use chrono::Utc;
use serde::{Serialize, Deserialize};

use crate::{v1::net::DistantNode, wallet::Hash256};

use super::{net::{PROTOCOL_VERSION, Node}, response::{Response}, state::State, transaction::Transaction};
use super::response::GetBlocksRes::UnknownHash;
use super::response::GetBlocksRes::DisconnectedChains;
use super::response::GetBlocksRes::BadChainIndex;
use super::response::GetBlocksRes::BadHashes;
use super::response::GetBlocksRes::Blocks;

const MAX_UNKNOWN_HASH_ATTEMPTS: usize = 3;

#[derive(Serialize, Deserialize, Debug)]
pub enum Request {
    GetAddr(GetAddrReq),
    Advertise(AdvertiseReq),
    GetBlocks(GetBlocksReq),
    NewTxn(Transaction)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetAddrReq {
    pub version: u32,
    pub addr_you: SocketAddr,
    pub listen_port: u16,
    pub best_height: usize,
    pub best_hash: Hash256
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AdvertiseReq {
    pub addr_me: SocketAddr
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetBlocksReq {
    pub your_hash: Hash256,
    pub my_hash: Hash256
}

pub fn get_first_peers(known_node: SocketAddr, state_mut: &Mutex<State>) -> Result<(), Box<dyn Error>> {
    let mut guard = state_mut.lock().unwrap();
    let state = &mut *guard;

    let (best_height, chain_idx, _) = state.blockchain.best_chain();

    let req = Request::GetAddr(GetAddrReq {
        version: PROTOCOL_VERSION,
        addr_you: known_node,
        listen_port: state.local_addr_me.port(),
        best_height,
        best_hash: state.blockchain.top_hash(chain_idx)
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
                best_hash: Some(data.best_hash)
            });

            state.network.known_nodes.push(DistantNode {
                addr: known_node
            });

            // TODO: Bootstrap with a few nodes to reduce the chances of a node lying about your remote IP
            state.remote_addr_me = Some(data.addr_you);
            state.network.clean(data.addr_you);

            Ok(())
        },
        _ => Err("Known node responded with nonsense")?
    }
}

pub fn discover(seed_addr: SocketAddr, state_mut: &Mutex<State>) -> Result<(), Box<dyn Error>> {
    let mut guard = state_mut.lock().unwrap();
    let state = &mut *guard;

    let addrs: Vec<SocketAddr> = 
        state.network.peers
            .iter()
            .filter(|n| *n != seed_addr)
            .map(|n| n.addr).collect();

    let (best_height, chain_idx, _) = state.blockchain.best_chain();

    for addr in addrs {
        let req = Request::GetAddr(GetAddrReq {
            version: PROTOCOL_VERSION,
            addr_you: addr,
            listen_port: state.local_addr_me.port(),
            best_height: best_height,
            best_hash: state.blockchain.top_hash(chain_idx)
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
            },
            // Remove nodes that return nonsense
            _ => state.network.remove(addr),
        }
    }

    for node in &state.network.peers {
        state.network.known_nodes.push(node.into());
    }

    let addr_me = state.remote_addr_me.unwrap();

    state.network.clean(addr_me);
    state.network.find_new_friends(state.port(), addr_me, best_height, state.blockchain.top_hash(chain_idx));

    Ok(())
}

pub fn download_latest_blocks(state_mut: &Mutex<State>) -> Result<(), Box<dyn Error>> {
    let mut guard = state_mut.lock().unwrap();
    let state = &mut *guard;

    let best_node_opt = state.network.most_updated_node();
    let best_node = match best_node_opt {
        None => {
            return Err("No suitable nodes to update local blockchain")?;
        },
        Some(node) => node
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
                    Blocks(mut blocks) => {
                        if blocks[0].header.prev_hash == hash {
                            // TODO VERIFY
                            state.blockchain.blocks.append(&mut blocks);
                        } else {
                            return Err("Received block with bad prev hash")?;
                        }
                        break;
                    },
                    UnknownHash(_) => {
                        block_idx = block_idx - 1;
                        hash = state.blockchain.blocks[block_idx - 1].header.hash;

                        println!("Received `UnknownHash` while trying to download blockchain");
                    },
                    DisconnectedChains => return Err("Tried to download blockchain across unconnected forks")?,
                    BadChainIndex => return Err("Tried to download blockchain with bad chain index")?,
                    BadHashes => return Err("Tried to download blockchain with bad hashes")?,
                    
                }
            },
            _ => {
                // TODO: Remove node and try again
                return Err("Peer node returned nonsense")?;
            }
        }

        attempt += 1;
    }

    println!("Up to date: {} blocks", state.blockchain.blocks.len());

    Ok(())
}

pub fn advertise_self(state_mut: &Mutex<State>) -> Result<(), Box<dyn Error>> {
    let guard = state_mut.lock().unwrap();
    let state = &*guard;
    let addr_me = state.remote_addr_me.unwrap();

    let req = Request::Advertise(AdvertiseReq {
        addr_me
    });

    state.network.broadcast(&req);

    Ok(())
}

pub fn send_req(req: &Request, addr: &SocketAddr) -> bincode::Result<Response> {
    let socket = TcpStream::connect(addr)?;
    bincode::serialize_into(&socket, &req)?;

    let res: Response = bincode::deserialize_from(&socket)?;

    Ok(res)
}
