use std::{sync::{Mutex, mpsc::{Receiver, Sender}}, error::Error};

use crate::{v1::state::State, fltk_helpers::dialog2};

pub enum GUIRequest {
    ProposeConnection(String)
}

pub enum GUIResponse {
    ProposeConnection(bool)
}

/// Process GUI requests in a loop. This function must be run on the main thread because FLTK operations can generally
/// only be done by the main thread. If a child thread needs to create a window, it should send a custom GUIRequest
/// and possibly expect a GUIResponse in the response channel.
pub fn gui_req_loop(req_receiver: Receiver<GUIRequest>, res_sender: Sender<GUIResponse>, _state_mut: &Mutex<State>) {
    loop {
        let gui_req = match req_receiver.recv() {
            Ok(req) => req,
            Err(_) => {
                println!("GUI request sender was disconnected");
                return;
            },
        };

        let result = match gui_req {
            GUIRequest::ProposeConnection(addr) => propose_connection(addr, &res_sender),
        };

        match result {
            Ok(_) => (),
            Err(err) => {
                println!("Error handling GUI request: {}", err);
            }
        }
    }
}

/// Creates a dialog box asking the user if they want to accept a connection request from another address.
fn propose_connection(sender_name: String, res_sender: &Sender<GUIResponse>) -> Result<(), Box<dyn Error>> {
    let is_accepted = dialog2(&format!("{} wants to connect", sender_name), "Accept", "Reject");

    res_sender.send(GUIResponse::ProposeConnection(is_accepted))?;

    Ok(())
}
