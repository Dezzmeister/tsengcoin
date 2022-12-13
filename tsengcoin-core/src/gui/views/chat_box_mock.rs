use std::sync::{Mutex, Arc};

use crate::{wallet::Address, v1::{state::State, chain_request::ChatSession}};

use super::MockWindow;

#[derive(Clone)]
pub struct ChatBoxUI {
    pub win: MockWindow
}

impl ChatBoxUI {
    pub fn new(sender: Address, sender_name: String, state_arc: &Arc<Mutex<State>>) -> Self {
        Self {
            win: MockWindow {}
        }
    }

    pub fn show(&mut self) {}
    pub fn hide(&mut self) {}
    pub fn shown(&mut self) -> bool {
        true
    }

    pub fn set_messages(&mut self, session: &ChatSession) {}

    pub fn add_message(&mut self, sender: &str, msg: &str) {}
}