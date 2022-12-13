use fltk::app::App;
use std::{
    error::Error,
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
};

use crate::{
    gui::{
        fltk_helpers::{dialog2, do_on_gui_thread},
        views::{
            main_box::{handle_messages, MainUI},
            BasicVisible,
        },
    },
    v1::state::State,
};

#[derive(Debug, Clone)]
pub struct GUIState {
    pub app: App,
    pub main_ui: MainUI,
}

/// GUI Requests are used for lightweight GUI tasks that don't require a main window - things such
/// as showing a dialog box and expecting a result from the user. If we're running in "nearly headless"
/// mode, this is the system to use; if not, use [do_on_gui_thread].
pub enum GUIRequest {
    ProposeConnection(String),
}

pub enum GUIResponse {
    ProposeConnection(bool),
}

impl GUIState {
    pub fn new() -> Self {
        Self {
            app: App::default(),
            main_ui: MainUI::new(),
        }
    }
}

/// Process GUI requests in a loop. This function must be run on the main thread because FLTK operations can generally
/// only be done by the main thread. If a child thread needs to create a light window, it should send a custom GUIRequest
/// and possibly expect a GUIResponse in the response channel.
///
/// If we're running in nearly headless mode, then this will be the request loop. There is no main window though,
/// so this is really only useful for small dialog boxes and popups. This also means that some features will be disabled
/// if we're running in nearly headless mode.
pub fn gui_req_loop(req_receiver: Receiver<GUIRequest>, res_sender: Sender<GUIResponse>) {
    loop {
        let gui_req = match req_receiver.recv() {
            Ok(req) => req,
            Err(_) => {
                println!("GUI request sender was disconnected");
                return;
            }
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

/// The main event loop for the application in GUI mode. As of 12/11/2022, GUI mode means that a main window
/// starts up alongside the console application. The console can be used exactly as in nearly headless mode; the window
/// allows some additional features to be used. On-chain chat requests can only be sent and received if a GUI
/// is present, so this is a feature that is not available in nearly headless mode.
pub fn main_gui_loop(state_arc: Arc<Mutex<State>>) {
    let mut gui = {
        let state_mut = &state_arc;
        let state = state_mut.lock().unwrap();
        state.gui.as_ref().unwrap().clone()
    };

    gui.main_ui.show();

    while gui.app.wait() {
        handle_messages(&state_arc, &gui.main_ui);
    }
}

pub fn is_connection_accepted(
    sender_name: String,
    req_channel: &Sender<GUIRequest>,
    res_channel: &Receiver<GUIResponse>,
    with_gui: bool,
) -> Result<bool, Box<dyn Error>> {
    if !with_gui {
        req_channel.send(GUIRequest::ProposeConnection(sender_name))?;

        return match res_channel.recv() {
            Ok(GUIResponse::ProposeConnection(was_accepted)) => Ok(was_accepted),
            _ => return Err("Error receving from GUI response channel".into()),
        };
    }

    let dialog_res = do_on_gui_thread(move || {
        dialog2(
            &format!("{} wants to connect", sender_name),
            "Accept",
            "Reject",
        )
    });

    match dialog_res {
        Ok(was_accepted) => Ok(was_accepted),
        Err(_) => Err("Error reading from temporary GUI channel".into()),
    }
}

/// Creates a dialog box asking the user if they want to accept a connection request from another address.
fn propose_connection(
    sender_name: String,
    res_sender: &Sender<GUIResponse>,
) -> Result<(), Box<dyn Error>> {
    let is_accepted = dialog2(
        &format!("{} wants to connect", sender_name),
        "Accept",
        "Reject",
    );

    res_sender.send(GUIResponse::ProposeConnection(is_accepted))?;

    Ok(())
}
