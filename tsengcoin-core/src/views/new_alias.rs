use std::sync::{Mutex, Arc};

use fltk::enums::{Align, LabelType, Color};
use fltk::prelude::{WidgetExt, WidgetBase, InputExt, WindowExt, GroupExt, DisplayExt};
use fltk::window::Window;
use fltk::input::Input;
use fltk::text::{TextDisplay, TextBuffer};
use fltk::button::Button;

use crate::v1::state::State;
use crate::wallet::b58c_to_address;
use basic_visible_derive::BasicVisible;
use crate::views::BasicVisible;

#[derive(BasicVisible)]
pub struct NewAliasUI {
    pub win: Window
}

impl NewAliasUI {
    pub fn new(state_arc: Arc<Mutex<State>>) -> Self {
        let mut win = Window::default().with_size(400, 150).with_label("New Alias");
        let mut save_btn = Button::new(325, 120, 64, 20, "Save");
        let mut cancel_btn = Button::new(255, 120, 64, 20, "Cancel");
        let mut address_input = Input::new(20, 32, 225, 22, "Address");
        address_input.set_align(Align::TopLeft);

        let mut alias_input = Input::new(20, 82, 225, 22, "Alias");
        alias_input.set_align(Align::TopLeft);

        let btn_state_arc = Arc::clone(&state_arc);
        let mut win_clone = win.clone();
        let mut win_clone_2 = win.clone();

        let mut error_display = TextDisplay::default().with_pos(20, 120).with_size(225, 23);
        error_display.set_label_type(LabelType::None);
        error_display.set_text_size(12);
        error_display.set_color(Color::from_hex(0xE0E0E0));
        error_display.set_text_color(Color::by_index(1));

        let mut error_buf = TextBuffer::default();
        error_buf.append("Invalid address\0");
        error_display.set_buffer(error_buf);

        error_display.hide();

        cancel_btn.set_callback(move |_| {
            win_clone_2.hide();
        });

        save_btn.set_callback(move |_| {
            let state_mut = &btn_state_arc;
            let mut state = state_mut.lock().unwrap();

            let address_b58c = address_input.value();
            let alias = alias_input.value();
            
            let address = match b58c_to_address(address_b58c) {
                Err(_) => {
                    error_display.show();
                    return;
                },
                Ok(addr) => addr
            };
            
            state.friends.aliases.insert(address, alias);

            win_clone.hide();
        });

        win.make_modal(true);
        win.end();

        Self {
            win
        }
    }
}
