use std::sync::{Mutex, Arc};

use basic_visible_derive::BasicVisible;
use crate::v1::chain_request::{make_encrypted_chain_req, ChatSession, ChatMessage};
use crate::v1::encrypted_msg::{ChainRequest, ChainChatReq};
use crate::v1::request::send_new_txn;
use crate::v1::state::State;
use crate::views::BasicVisible;
use crate::wallet::Address;
use fltk::enums::{LabelType, Color, Align};
use fltk::prelude::{WidgetExt, GroupExt, InputExt};
use fltk::window::Window;
use fltk::output::MultilineOutput;
use fltk::input::Input;
use fltk::button::ReturnButton;
use fltk::group::{Group};

const TRUNCATE_AFTER: usize = 10;

#[derive(BasicVisible, Clone)]
pub struct ChatBoxUI {
    pub win: Window,
    pub output: MultilineOutput,
    pub input: Input,
    pub send_btn: ReturnButton
}

impl ChatBoxUI {
    pub fn new(sender: Address, sender_name: String, state_arc: &Arc<Mutex<State>>) -> Self {
        let mut win = Window::default().with_label(&format!("Chat with {}", &sender_name)).with_size(400, 300);
        let whole_group = Group::default().with_pos(0, 0).with_size(400, 300);

        // TODO: Scrollbar
        let mut scrollbar = Group::default().with_pos(0, 0).with_size(400, 260);

        let mut output = MultilineOutput::default().with_pos(0, 0).with_size(400, 260);
        output.set_label_type(LabelType::None);
        output.set_color(Color::by_index(46));
        output.set_align(Align::TopLeft);
        output.set_wrap(true);

        scrollbar.add(&output);
        scrollbar.resizable(&output);
        scrollbar.end();

        let bottom_group = Group::default().with_pos(0, 270).with_size(400, 30);

        let mut input = Input::default().with_pos(0, 270).with_size(330, 30);
        input.set_label_type(LabelType::None);

        let btn_state_arc = Arc::clone(state_arc);
        let mut input_clone = input.clone();
        let mut output_clone = output.clone();

        let mut button = ReturnButton::default().with_pos(330, 270).with_size(70, 30).with_label("Send");
        button.set_color(Color::by_index(230));

        button.set_callback(move |_| {
            let mut state = btn_state_arc.lock().unwrap();

            let msg_out = input_clone.value();
            if !is_valid_message(&msg_out) {
                return;
            }

            let chain_req = ChainRequest::ChainChat(ChainChatReq {
                msg: msg_out.clone()
            });

            let enc_req = match make_encrypted_chain_req(chain_req, sender, &mut state) {
                Ok(req) => req,
                Err(err) => {
                    println!("Error making encrypted chain request: {}", err);
                    return;
                }
            };

            match send_new_txn(enc_req, &state) {
                Ok(_) => (),
                Err(err) => {
                    println!("Error sending chain request: {}", err);
                    return;
                }
            };

            add_message_to_history(&mut output_clone, "You", &msg_out);

            let session = state.friends.chat_sessions.get_mut(&sender_name).unwrap();
            session.messages.push(ChatMessage {
                sender: String::from("You"),
                message: msg_out
            });

            input_clone.set_value("");
        });

        bottom_group.resizable(&input);
        bottom_group.end();

        whole_group.resizable(&scrollbar);
        whole_group.end();

        win.resizable(&whole_group);
        win.make_resizable(true);

        Self {
            win,
            output,
            input,
            send_btn: button
        }
    }
    
    pub fn set_messages(&mut self, session: &ChatSession) {
        let txt = chat_session_to_multiline(session);
        self.output.set_value(&txt);
    }

    pub fn add_message(&mut self, sender: &str, msg: &str) {
        add_message_to_history(&mut self.output, sender, msg);
    }
}

fn chat_line(sender: &str, message: &str) -> String {
    format!("{}:\t{}\n", truncate_addr(sender), message)
}

fn chat_session_to_multiline(chat: &ChatSession) -> String {
    let mut out = String::from("");

    for message in &chat.messages {
        out.push_str(&chat_line(&message.sender, &message.message))
    }

    out
}

fn truncate_addr(addr: &str) -> String {
    if addr.len() <= TRUNCATE_AFTER {
        return addr.to_owned();
    }

    let mut out = addr[0..TRUNCATE_AFTER].to_owned();
    out.push_str("..");

    out
}

fn add_message_to_history(output: &mut MultilineOutput, sender: &str, msg: &str) {
    let mut current_history = output.value();
    current_history.push_str(&chat_line(sender, msg));

    output.set_value(&current_history);
}

fn is_valid_message(msg: &str) -> bool {
    !msg.trim().is_empty()
}
