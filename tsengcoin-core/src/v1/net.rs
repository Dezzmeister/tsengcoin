use std::{
    cmp::min,
    error::Error,
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
    },
};

use chrono::{DateTime, Utc};
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

    pub fn prune_dead_nodes(&mut self, broadcast_result: &mut [usize]) {
        broadcast_result.sort_unstable();

        let mut known_poses: Vec<usize> = vec![];

        for pos in broadcast_result.iter().rev() {
            let dead_node = self.peers.remove(*pos);
            if let Some(known_pos) = self.known_nodes.iter().position(|k| k == &dead_node) {
                known_poses.push(known_pos);
            }
        }

        known_poses.sort();

        for pos in known_poses.iter().rev() {
            self.known_nodes.remove(*pos);
        }
    }

    /// Pick new peers at random from the list of known peers. If the network is large enough then we
    /// choose [MAX_NEIGHBORS] peers; if not, we choose all known nodes as peers. Then we send each prospective peer
    /// a 'GetAddr' request to get some crucial info. There may be several nodes, so this step is done in parallel.
    /// We then wait for and collect the responses to these requests and loop over them. For any bad response, we
    /// drop the node from our list of known nodes. We keep the good responses and use them as our peers.
    pub fn find_new_friends(
        &mut self,
        listen_port: u16,
        addr_me: SocketAddr,
        best_height: usize,
        best_hash: Hash256,
    ) {
        // Move the known nodes around then take the first nodes from 0 to `num_peers`. These will be our
        // prospective peers - we'll send GetAddrs and handle the results accordingly.
        self.shuffle();
        let num_peers = min(self.known_nodes.len(), MAX_NEIGHBORS);
        let new_peers = self.known_nodes[0..num_peers]
            .iter()
            .map(|n| n.addr)
            .collect::<Vec<SocketAddr>>();

        let results = crossbeam::scope(|scope| {
            new_peers
                .iter()
                .map(|addr| {
                    scope.spawn(move |_| {
                        let req = Request::GetAddr(GetAddrReq {
                            version: PROTOCOL_VERSION,
                            addr_you: *addr,
                            listen_port,
                            best_height,
                            best_hash,
                        });

                        send_req(&req, addr)
                    })
                })
                .map(|t| t.join().unwrap())
                .collect::<Vec<Result<Response, Box<bincode::ErrorKind>>>>()
        })
        .unwrap();

        self.peers.clear();

        for i in 0..results.len() {
            let result = &results[i];

            match result {
                Ok(Response::GetAddr(data)) => {
                    let addr_you = new_peers[i];
                    let node = Node {
                        version: data.version,
                        addr: addr_you,
                        last_send: Utc::now(),
                        best_height: Some(data.best_height),
                        best_hash: Some(data.best_hash),
                    };

                    self.peers.push(node);

                    let mut neighbors = data
                        .neighbors
                        .iter()
                        .map(|n| n.into())
                        .collect::<Vec<DistantNode>>();
                    self.known_nodes.append(&mut neighbors);
                }

                // Do not accept bogus
                Ok(_) | Err(_) => drop(self.known_nodes.remove(i)),
            }
        }

        self.clean(addr_me);
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

                // If something goes wrong here, we want to panic. Nothing should go wrong here
                handle_request(req, &conn, gui_channels, state_arc)?;
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

                handle_request(req, &conn, gui_channels, state_arc)?;
            }
        }
    }

    Ok(())
}
