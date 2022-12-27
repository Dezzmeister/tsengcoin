use std::sync::{Arc, Mutex};

use fltk::{
    button::Button,
    enums::{Align, Color, LabelType},
    input::Input,
    prelude::{DisplayExt, GroupExt, InputExt, WidgetBase, WidgetExt, WindowExt},
    text::{TextBuffer, TextDisplay},
    window::Window,
};

use crate::{gui::views::BasicVisible, v1::state::State, wallet::b58c_to_address};
use basic_visible_derive::BasicVisible;

#[derive(BasicVisible)]
pub struct NewCustomTxnUI {
    pub win: Window,
}

impl NewCustomTxnUI {
    pub fn new(state_arc: Arc<Mutex<State>>) -> Self {
        todo!()
    }
}
