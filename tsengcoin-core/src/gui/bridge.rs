use crate::{v1::state::GUIChannels, command::CommandInvocation};
use std::error::Error;

#[cfg(feature = "gui")]
pub fn is_connection_accepted(
    sender_name: String,
    channels: &GUIChannels,
    with_gui: bool,
    _default: bool,
) -> Result<bool, Box<dyn Error>> {
    use crate::gui::{gui::GUIResponse, fltk_helpers::{do_on_gui_thread, dialog2}};

    use super::gui::GUIRequest;

    if !with_gui {
        channels.req_channel.send(GUIRequest::ProposeConnection(sender_name))?;

        return match channels.res_channel.recv() {
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

#[cfg(not(feature = "gui"))]
pub fn is_connection_accepted(
    _sender_name: String,
    _channels: &GUIChannels,
    _with_gui: bool,
    default: bool
) -> Result<bool, Box<dyn Error>> {
    Ok(default)
}

#[cfg(feature = "gui")]
pub fn get_wallet_password_arg(invocation: &CommandInvocation) -> String {
    invocation.get_field("wallet-password")
    .unwrap_or_else(|| {
        fltk::dialog::password_default("Enter your wallet password", "")
            .expect("Need to supply a password!")
    })
}

#[cfg(not(feature = "gui"))]
pub fn get_wallet_password_arg(invocation: &CommandInvocation) -> String {
    invocation.get_field("wallet-password").unwrap()
}
