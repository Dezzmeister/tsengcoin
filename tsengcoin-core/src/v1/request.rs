use std::{net::{SocketAddr, TcpStream}, error::Error, sync::{Mutex}};

use chrono::Utc;
use serde::{Serialize, Deserialize};

use crate::v1::net::DistantNode;

use super::{net::{PROTOCOL_VERSION, Node}, response::{Response}, state::State};

#[derive(Serialize, Deserialize, Debug)]
pub enum Request {
    GetAddr(GetAddrReq),
    Advertise(AdvertiseReq)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetAddrReq {
    pub version: u32,
    pub addr_you: SocketAddr,
    pub listen_port: u16,
    pub best_height: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AdvertiseReq {
    pub addr_me: SocketAddr
}

pub fn get_first_peers(known_node: SocketAddr, state_mut: &Mutex<State>) -> Result<(), Box<dyn Error>> {
    let mut guard = state_mut.lock().unwrap();
    let state = &mut *guard;

    let req = Request::GetAddr(GetAddrReq {
        version: PROTOCOL_VERSION,
        addr_you: known_node,
        listen_port: state.local_addr_me.port(),
        best_height: 1
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

    for addr in addrs {
        let req = Request::GetAddr(GetAddrReq {
            version: PROTOCOL_VERSION,
            addr_you: addr,
            listen_port: state.local_addr_me.port(),
            best_height: 1,
        });

        let result = send_req(&req, &addr);

        match result {
            Err(_) => state.network.remove(addr),
            Ok(Response::GetAddr(mut data)) => {
                state.network.peers.append(&mut data.neighbors);
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
    state.network.find_new_friends(state.port(), addr_me, state.best_height);

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
