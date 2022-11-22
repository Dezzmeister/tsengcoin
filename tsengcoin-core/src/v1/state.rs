use std::{net::SocketAddr, sync::{Arc, Mutex}};

use super::net::Network;

#[derive(Debug)]
pub struct State {
    pub best_height: u64,
    pub local_addr_me: SocketAddr,
    pub remote_addr_me: Option<SocketAddr>,
    pub network: Network
}

pub type ThreadsafeState = Arc<Mutex<State>>;

impl State {
    pub fn new(addr_me: SocketAddr) -> Self {
        Self {
            best_height: 1,
            local_addr_me: addr_me,
            remote_addr_me: None,
            network: Network {
                peers: vec![],
                known_nodes: vec![],
            }
        }
    }

    pub fn port(&self) -> u16 {
        self.local_addr_me.port()
    }
}
