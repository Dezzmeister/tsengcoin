use std::{net::{TcpStream, SocketAddr}, error::Error, sync::{Mutex}};

use chrono::Utc;
use serde::{Serialize, Deserialize};

use super::{request::{Request, GetAddrReq, AdvertiseReq}, state::{State}, net::{PROTOCOL_VERSION, Node, DistantNode}};

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    GetAddr(GetAddrRes),
    AdvertiseRes
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetAddrRes {
    pub version: u32,
    pub addr_you: SocketAddr,
    pub neighbors: Vec<Node>,
}

pub fn handle_request(req: Request, socket: &TcpStream, state_mut: &Mutex<State>) -> Result<(), Box<dyn Error>> {
    match req {
        Request::GetAddr(data) => handle_get_addr(data, socket, state_mut),
        Request::Advertise(data) => handle_advertise(data, socket, state_mut),
    }
}

fn handle_get_addr(data: GetAddrReq, socket: &TcpStream, state_mut: &Mutex<State>) -> Result<(), Box<dyn Error>> {
    let peer_remote_addr = socket.peer_addr().unwrap().ip();
    let peer_remote_port = data.listen_port;
    let addr_you = SocketAddr::new(peer_remote_addr, peer_remote_port);

    let mut guard = state_mut.lock().unwrap();
    let state = &mut *guard;

    let neighbors: Vec<Node> = state.network.peers.iter().map(|p| p.to_owned()).collect();

    let res = Response::GetAddr(GetAddrRes {
        version: PROTOCOL_VERSION,
        addr_you,
        neighbors
    });

    let node = Node {
        version: data.version,
        addr: addr_you,
        last_send: Utc::now(),
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

    send_res(Response::AdvertiseRes, socket)?;

    let mut guard = state_mut.lock().unwrap();
    let state = &mut *guard;

    let addr_me = state.remote_addr_me.unwrap();

    if state.network.has_known(addr_you) || (addr_you == addr_me) {
        return Ok(());
    }

    state.network.known_nodes.push(DistantNode {
        addr: addr_you
    });

    state.network.broadcast(&Request::Advertise(data));

    if rand::random::<u8>() % 2 == 0 {
        state.network.find_new_friends(state.port(), state.remote_addr_me.unwrap(), state.best_height);
    }

    Ok(())
}

pub fn send_res(res: Response, stream: &TcpStream) -> bincode::Result<()> {
    bincode::serialize_into(stream, &res)
}
