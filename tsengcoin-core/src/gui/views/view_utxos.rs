use std::sync::{Arc, Mutex};

use fltk::{
    button::Button,
    enums::{Align, Color, LabelType},
    input::{Input, IntInput},
    prelude::{DisplayExt, InputExt, WidgetBase, WidgetExt, WindowExt, GroupExt},
    text::{TextBuffer, TextDisplay},
    window::Window,
};
use ring::signature::KeyPair;

use crate::{
    gui::views::BasicVisible,
    v1::{
        request::send_new_txn,
        state::State, transaction::{make_single_p2pkh_txn, sign_txn, make_p2pkh_unlock, TxnInput, UnhashedTransaction, hash_txn}, VERSION, txn_verify::verify_transaction,
    },
};
use basic_visible_derive::BasicVisible;

#[derive(BasicVisible)]
pub struct ViewUTXOsUI {
    pub win: Window,
}

impl ViewUTXOsUI {
    pub fn new(state_arc: Arc<Mutex<State>>) -> Self {
        todo!()
    }
}
