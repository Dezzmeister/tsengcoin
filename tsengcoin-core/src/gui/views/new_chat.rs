use std::sync::{Arc, Mutex};

use fltk::{
    button::Button,
    enums::{Align, Color, LabelType},
    input::{Input, IntInput},
    prelude::{DisplayExt, GroupExt, InputExt, WidgetBase, WidgetExt, WindowExt},
    text::{TextBuffer, TextDisplay},
    window::Window,
};

use crate::{
    gui::views::BasicVisible,
    v1::{
        chain_request::make_dh_connect_req,
        encrypted_msg::{ChainChatReq, ChainRequest},
        request::send_new_txn,
        state::State,
    },
};
use basic_visible_derive::BasicVisible;

#[derive(BasicVisible)]
pub struct NewChatUI {
    pub win: Window,
}

impl NewChatUI {
    pub fn new(state_arc: Arc<Mutex<State>>) -> Self {
        let chain_req_amount = {
            let state_mut = &state_arc;
            let state = state_mut.lock().unwrap();

            state.friends.chain_req_amount
        };

        let mut win = Window::default().with_size(400, 200).with_label("New Chat");
        let mut send_btn = Button::new(325, 170, 64, 20, "Send");
        let mut cancel_btn = Button::new(255, 170, 64, 20, "Cancel");
        let mut address_input = Input::new(20, 32, 225, 22, "Address/Alias");
        address_input.set_align(Align::TopLeft);

        let mut chain_req_input = IntInput::new(20, 82, 80, 22, "Request Amount (TGC)");
        chain_req_input.set_value(&format!("{}", chain_req_amount));
        chain_req_input.set_align(Align::TopLeft);

        let mut first_message_input = Input::new(20, 132, 225, 22, "First Message");
        first_message_input.set_align(Align::TopLeft);

        let mut error_display = TextDisplay::default().with_pos(20, 170).with_size(225, 23);
        error_display.set_label_type(LabelType::None);
        error_display.set_text_size(12);
        error_display.set_color(Color::from_hex(0xE0E0E0));
        error_display.set_text_color(Color::by_index(1));

        let mut error_buf = TextBuffer::default();
        error_buf.append("Invalid address\0");
        error_display.set_buffer(error_buf);

        error_display.hide();

        let btn_state_arc = Arc::clone(&state_arc);
        let mut win_clone = win.clone();
        let mut win_clone_2 = win.clone();

        cancel_btn.set_callback(move |_| {
            win_clone_2.hide();
        });

        send_btn.set_callback(move |_| {
            let state_mut = &btn_state_arc;
            let mut state = state_mut.lock().unwrap();

            let first_message = first_message_input.value();
            let req_amount = chain_req_input
                .value()
                .parse::<u64>()
                .unwrap_or(state.friends.chain_req_amount);
            let dest_address = match state.friends.get_address(address_input.value()) {
                Err(_) => {
                    error_display.show();
                    return;
                }
                Ok(addr) => addr,
            };

            let intent = ChainRequest::ChainChat(ChainChatReq { msg: first_message });

            let connect_req =
                match make_dh_connect_req(dest_address, req_amount, 1, Some(intent), &mut state) {
                    Err(err) => {
                        println!("Error making chat transaction: {}", err);
                        win_clone.hide();
                        return;
                    }
                    Ok(req) => req,
                };

            match send_new_txn(connect_req, &mut state) {
                Err(err) => println!("Error sending chat transaction: {}", err),
                Ok(_) => {
                    println!("Sent chat request");
                }
            };

            win_clone.hide();
        });

        win.make_modal(true);
        win.end();

        Self { win }
    }
}
