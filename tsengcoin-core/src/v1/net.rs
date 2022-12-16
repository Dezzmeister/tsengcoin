use std::{
    cmp::min,
    error::Error,
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
    },
};

use chrono::{DateTime, Utc};
use crossbeam::thread::{ScopedJoinHandle};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::wallet::Hash256;

use super::{
    request::{send_msg, send_req, GetAddrReq, Request},
    response::{handle_request, Response},
    state::State,
};
use super::state::GUIChannels;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_NEIGHBORS: usize = 8;
pub const MAX_GET_ADDRS: usize = 3;

#[derive(Debug, Clone)]
pub struct DistantNode {
    pub addr: SocketAddr,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Node {
    pub version: u32,
    pub addr: SocketAddr,
    pub last_send: DateTime<Utc>,
    pub best_height: Option<usize>,
    pub best_hash: Option<Hash256>,
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hash_debug = self.best_hash.map(hex::encode);

        f.debug_struct("Node")
            .field("version", &self.version)
            .field("addr", &self.addr)
            .field("last_send", &self.last_send)
            .field("best_height", &self.best_height)
            .field("best_hash", &hash_debug)
            .finish()
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.addr == other.addr
    }
}

impl PartialEq<SocketAddr> for Node {
    fn eq(&self, other: &SocketAddr) -> bool {
        self.addr == *other
    }
}

impl PartialEq<SocketAddr> for &Node {
    fn eq(&self, other: &SocketAddr) -> bool {
        (*self).addr == (*other)
    }
}

impl DistantNode {
    pub fn send_req(&self, req: Request) -> Result<Response, Box<dyn Error>> {
        let stream = TcpStream::connect(self.addr)?;
        bincode::serialize_into(&stream, &req)?;

        let res: Response = bincode::deserialize_from(&stream)?;

        Ok(res)
    }

    pub fn send_res(&self, res: Response) -> Result<(), Box<dyn Error>> {
        let stream = TcpStream::connect(self.addr)?;
        bincode::serialize_into(&stream, &res)?;

        Ok(())
    }
}

impl From<&Node> for DistantNode {
    fn from(node: &Node) -> Self {
        DistantNode { addr: node.addr }
    }
}

impl From<Node> for DistantNode {
    fn from(node: Node) -> Self {
        DistantNode { addr: node.addr }
    }
}

impl PartialEq for DistantNode {
    fn eq(&self, other: &Self) -> bool {
        self.addr == other.addr
    }
}
impl PartialEq<SocketAddr> for &DistantNode {
    fn eq(&self, other: &SocketAddr) -> bool {
        self.addr == *other
    }
}

impl PartialEq<SocketAddr> for DistantNode {
    fn eq(&self, other: &SocketAddr) -> bool {
        self.addr == *other
    }
}

impl PartialEq<Node> for DistantNode {
    fn eq(&self, other: &Node) -> bool {
        self.addr == other.addr
    }
}

impl PartialEq<&mut SocketAddr> for Node {
    fn eq(&self, other: &&mut SocketAddr) -> bool {
        &&self.addr == other
    }
}

impl PartialEq<&mut SocketAddr> for DistantNode {
    fn eq(&self, other: &&mut SocketAddr) -> bool {
        &&self.addr == other
    }
}

#[derive(Debug)]
pub struct Network {
    pub peers: Vec<Node>,
    pub known_nodes: Vec<DistantNode>,
}

impl Network {
    pub fn remove<T: PartialEq>(&mut self, node: T)
    where
        Node: PartialEq<T>,
        DistantNode: PartialEq<T>,
    {
        if let Some(pos) = self.peers.iter().position(|n| *n == node) {
            drop(self.peers.remove(pos));
        }

        if let Some(pos) = self.known_nodes.iter().position(|n| *n == node) {
            drop(self.known_nodes.remove(pos));
        }
    }

    pub fn clean<T: PartialEq>(&mut self, me: T)
    where
        Node: PartialEq<T>,
        DistantNode: PartialEq<T>,
    {
        self.remove(me);
        self.peers.dedup();
        self.known_nodes.dedup();
    }

    pub fn shuffle(&mut self) {
        let rng = &mut rand::thread_rng();

        self.peers.shuffle(rng);
        self.known_nodes.shuffle(rng);
    }

    pub fn broadcast_msg(&self, msg: &Request) -> Vec<usize> {
        self.peers
            .iter()
            .enumerate()
            .filter_map(|(i, n)| {
                match send_msg(msg, &n.addr) {
                    Ok(_) => None,
                    Err(_) => Some(i)
                }
            })
            .collect::<Vec<usize>>()
    }

    pub fn peer_addrs(&self) -> Vec<SocketAddr> {
        self.peers.iter().map(|n| n.addr).collect::<Vec<SocketAddr>>()
    }

    pub fn prune_dead_nodes(&mut self, broadcast_result: &mut [SocketAddr]) {
        for addr in broadcast_result.into_iter() {
            self.remove(addr);
        }
    }

    pub fn has_peer<T: PartialEq>(&self, item: T) -> bool
    where
        Node: PartialEq<T>,
    {
        self.peers.iter().any(|n| *n == item)
    }

    pub fn has_known<T: PartialEq>(&self, item: T) -> bool
    where
        DistantNode: PartialEq<T>,
    {
        self.known_nodes.iter().any(|n| *n == item)
    }

    /// Get the node with the best block height. Returns None if there are no nodes
    /// with a block height greater than zero.
    pub fn most_updated_node<'a>(&'a self) -> Option<&'a Node> {
        let mut best_height: usize = 0;
        let mut best_node: Option<&'a Node> = None;

        for node in &self.peers {
            let best_height_opt = node.best_height;

            if let Some(node_best_height) = best_height_opt {
                if node_best_height > best_height {
                    best_height = node_best_height;
                    best_node = Some(node);
                }
            }
        }

        best_node
    }

    fn merge(&mut self, addr_me: SocketAddr) {
        for node in &self.peers {
            self.known_nodes.push(DistantNode {
                addr: node.addr
            });
        }

        self.clean(addr_me);
    }
}

/// Pick new peers at random from the list of known peers. If the network is large enough then we
/// choose [MAX_NEIGHBORS] peers; if not, we choose all known nodes as peers. Then we send each prospective peer
/// a 'GetAddr' request to get some crucial info. There may be several nodes, so this step is done in parallel.
/// We then wait for and collect the responses to these requests and loop over them. For any bad response, we
/// drop the node from our list of known nodes. We keep the good responses and use them as our peers.
pub fn find_new_friends(state_mut: &Mutex<State>) {
    let mut guard = state_mut.lock().unwrap();
    let state = &mut *guard;
    let addr_me = state.remote_addr_me.unwrap();
    let (best_height, chain_idx, _) = state.blockchain.best_chain();
    let best_hash = state.blockchain.top_hash(chain_idx);
    let listen_port = state.port();

    state.network.merge(addr_me);
    state.network.shuffle();
    let num_get_addrs = min(state.network.known_nodes.len(), MAX_GET_ADDRS);
    let get_addr_addrs = state.network.known_nodes[0..num_get_addrs]
        .iter()
        .map(|n| n.addr)
        .collect::<Vec<SocketAddr>>();
    
    // Release the mutex while waiting for responses so we don't hold up the other threads
    drop(guard);

    let get_addr_responses = broadcast_async_req_fn(|addr| {
        Request::GetAddr(GetAddrReq {
            version: PROTOCOL_VERSION,
            addr_you: addr,
            listen_port,
            best_height,
            best_hash,
        })
    }, &get_addr_addrs);

    let mut guard = state_mut.lock().unwrap();
    let state = &mut *guard;

    for (res_opt, addr) in get_addr_responses {
        if res_opt.is_none() {
            state.network.remove(addr);
            continue;
        }

        match res_opt.unwrap() {
            Response::GetAddr(data) => {
                let node = Node {
                    version: data.version,
                    addr,
                    last_send: Utc::now(),
                    best_height: Some(data.best_height),
                    best_hash: Some(data.best_hash),
                };

                state.network.peers.push(node);

                let mut neighbors = data
                    .neighbors
                    .iter()
                    .map(|n| n.into())
                    .collect::<Vec<DistantNode>>();
                state.network.known_nodes.append(&mut neighbors);
            },
            _ => state.network.remove(addr)
        };
    }

    state.network.clean(addr_me);
}

pub fn broadcast_async_req_fn<F>(req_fn: F, peers: &[SocketAddr]) -> Vec<(Option<Response>, SocketAddr)>
    where F: Fn(SocketAddr) -> Request
{
    crossbeam::scope(|scope| {
        let join_handles = peers
            .iter()
            .map(|addr| {
                let req = req_fn(addr.clone());
                scope.spawn(move |_| {
                    let res = match send_req(&req, &addr) {
                        Ok(data) => Some(data),
                        Err(_) => None
                    };

                    (res, addr)
                })
            })
            .collect::<Vec<ScopedJoinHandle<(Option<Response>, &SocketAddr)>>>();

        join_handles
            .into_iter()
            .map(|j| {
                let (res, addr) = j.join().unwrap();

                (res, addr.clone())
            })
            .collect::<Vec<(Option<Response>, SocketAddr)>>()
    }).unwrap()
}

pub fn broadcast_async_req(req: Request, peers: &[SocketAddr]) -> Vec<(Option<Response>, SocketAddr)> {
    let req_arc = Arc::new(req);

    crossbeam::scope(|scope| {
        let join_handles = peers
            .iter()
            .map(|addr| {
                let req_arc_clone = Arc::clone(&req_arc);
                scope.spawn(move |_| {
                    let res = match send_req(&req_arc_clone, &addr) {
                        Ok(data) => Some(data),
                        Err(_) => None
                    };

                    (res, addr)
                })
            })
            .collect::<Vec<ScopedJoinHandle<(Option<Response>, &SocketAddr)>>>();

        join_handles
            .into_iter()
            .map(|j| {
                let (res, addr) = j.join().unwrap();

                (res, addr.clone())
            })
            .collect::<Vec<(Option<Response>, SocketAddr)>>()
    }).unwrap()
}

pub fn broadcast_async(msg: Request, peers: &[SocketAddr]) -> Vec<SocketAddr> {
    let msg_arc = Arc::new(msg);

    crossbeam::scope(|scope| {
        let join_handles = peers
            .iter()
            .map(|addr| {
                let msg_arc_clone = Arc::clone(&msg_arc);
                scope.spawn(move |_| {
                    let res = send_msg(&msg_arc_clone, &addr);

                    (addr, res.is_err())
                })
            })
            .collect::<Vec<ScopedJoinHandle<(&SocketAddr, bool)>>>();

        join_handles
            .into_iter()
            .filter_map(|j| {
                let (a, ok) = j.join().unwrap();
                match ok {
                    // Node is not dead
                    false => None,
                    // Node is dead
                    true => Some(a.clone())
                }
            })
            .collect::<Vec<SocketAddr>>()
    }).unwrap()
}

#[cfg(feature = "gui")]
pub fn listen_for_connections(
    listen_addr: SocketAddr,
    gui_channels: &GUIChannels,
    state_arc: &Arc<Mutex<State>>,
) -> Result<(), Box<dyn Error>> {
    let socket = TcpListener::bind(listen_addr)?;

    for stream in socket.incoming() {
        match stream {
            Err(err) => println!("Error receiving incoming connection: {}", err),
            Ok(conn) => {
                let req: Request = match bincode::deserialize_from(&conn) {
                    Ok(data) => data,
                    Err(err) => {
                        println!("Received invalid request over TCP: {}", err);
                        continue;
                    }
                };

                if let Err(err) = handle_request(req, conn, gui_channels, state_arc) {
                    println!("Error handling request: {}", err);
                }
            }
        }
    }

    Ok(())
}

#[cfg(not(feature = "gui"))]
pub fn listen_for_connections(
    listen_addr: SocketAddr,
    gui_channels: &GUIChannels,
    state_arc: &Arc<Mutex<State>>,
) -> Result<(), Box<dyn Error>> {
    let socket = TcpListener::bind(listen_addr)?;

    for stream in socket.incoming() {
        match stream {
            Err(err) => println!("Error receiving incoming connection: {}", err),
            Ok(conn) => {
                let req: Request = match bincode::deserialize_from(&conn) {
                    Ok(data) => data,
                    Err(err) => {
                        println!("Received invalid request over TCP: {}", err);
                        continue;
                    }
                };

                if let Err(err) = handle_request(req, conn, gui_channels, state_arc) {
                    println!("Error handling request: {}", err);
                }
            }
        }
    }

    Ok(())
}
