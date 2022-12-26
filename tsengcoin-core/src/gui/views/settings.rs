use std::sync::{Arc, Mutex};

use fltk::{
    button::Button,
    enums::Align,
    input::IntInput,
    prelude::{GroupExt, InputExt, WidgetBase, WidgetExt, WindowExt},
    window::Window,
};

use crate::{gui::views::BasicVisible, v1::state::State};
use basic_visible_derive::BasicVisible;

#[derive(BasicVisible)]
pub struct SettingsUI {
    pub win: Window,
}

impl SettingsUI {
    pub fn new(state_arc: Arc<Mutex<State>>) -> Self {
        let (exclusivity, chain_req_amount, default_fee) = {
            let state_mut = &state_arc;
            let state = state_mut.lock().unwrap();

            (state.friends.exclusivity, state.friends.chain_req_amount, state.default_fee)
        };

        let mut win = Window::default().with_size(400, 200).with_label("Settings");
        let mut save_btn = Button::new(325, 170, 64, 20, "Save");
        let mut cancel_btn = Button::new(255, 170, 64, 20, "Cancel");
        let mut chain_req_input = IntInput::new(20, 32, 80, 22, "Chain Request Amount (TGC)");
        chain_req_input.set_value(&format!("{}", chain_req_amount));
        chain_req_input.set_align(Align::TopLeft);

        let mut exclusivity_input = IntInput::new(20, 82, 80, 22, "Exclusivity (TGC)");
        exclusivity_input.set_value(&exclusivity_to_ui(exclusivity));
        exclusivity_input.set_align(Align::TopLeft);

        let mut default_fee_input = IntInput::new(20, 132, 80, 22, "Default Fee (TGC)");
        default_fee_input.set_value(&format!("{}", default_fee));
        default_fee_input.set_align(Align::TopLeft);

        let btn_state_arc = Arc::clone(&state_arc);
        let mut win_clone = win.clone();
        let mut win_clone_2 = win.clone();

        cancel_btn.set_callback(move |_| {
            win_clone_2.hide();
        });

        save_btn.set_callback(move |_| {
            let state_mut = &btn_state_arc;
            let mut state = state_mut.lock().unwrap();

            let new_chain_amount = chain_req_input.value().parse::<u64>().unwrap_or(1);
            let new_exclusivity = ui_to_exclusivity(&exclusivity_input.value());
            let new_default_fee = default_fee_input.value().parse::<u64>().unwrap_or(1);

            state.friends.chain_req_amount = new_chain_amount;
            state.friends.exclusivity = new_exclusivity;
            state.default_fee = new_default_fee;

            win_clone.hide();
        });

        win.make_modal(true);
        win.end();

        Self { win }
    }
}

fn ui_to_exclusivity(ui: &str) -> u64 {
    let val = ui.parse::<i64>().unwrap_or(1);
    match val < 0 {
        true => u64::MAX,
        false => val as u64,
    }
}

fn exclusivity_to_ui(exclusivity: u64) -> String {
    match exclusivity == u64::MAX {
        false => format!("{}", exclusivity),
        true => String::from("-1"),
    }
}
