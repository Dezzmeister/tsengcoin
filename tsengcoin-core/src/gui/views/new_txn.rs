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
pub struct NewTxnUI {
    pub win: Window,
}

impl NewTxnUI {
    pub fn new(state_arc: Arc<Mutex<State>>) -> Self {
        let default_fee = state_arc.lock().unwrap().default_fee;

        let mut win = Window::default().with_size(400, 250).with_label("New Transaction");
        let mut send_btn = Button::new(325, 220, 64, 20, "Send");
        let mut cancel_btn = Button::new(255, 220, 64, 20, "Cancel");
        let mut address_input = Input::new(20, 32, 225, 22, "Address/Alias");
        address_input.set_align(Align::TopLeft);

        let mut txn_amount_input = IntInput::new(20, 82, 80, 22, "Transaction Amount (TGC)");
        txn_amount_input.set_align(Align::TopLeft);

        let mut txn_fee_input = IntInput::new(20, 132, 80, 22, "Transaction Fee (TGC)");
        txn_fee_input.set_value(&format!("{}", default_fee));
        txn_fee_input.set_align(Align::TopLeft);

        let mut meta_input = Input::new(20, 182, 225, 22, "Metadata");
        meta_input.set_align(Align::TopLeft);

        let mut error_display = TextDisplay::default().with_pos(20, 220).with_size(225, 23);
        error_display.set_label_type(LabelType::None);
        error_display.set_text_size(12);
        error_display.set_color(Color::from_hex(0xE0E0E0));
        error_display.set_text_color(Color::by_index(1));

        let mut error_buf = TextBuffer::default();
        error_buf.append("Invalid or missing input(s)\0");
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
            let state = &mut state_mut.lock().unwrap();

            let dest_address = match state.friends.get_address(address_input.value()) {
                Ok(addr) => addr,
                Err(_) => {
                    error_display.show();
                    return;
                }
            };

            let txn_amount = match txn_amount_input.value().parse::<u64>() {
                Ok(amount) => amount,
                Err(_) => {
                    error_display.show();
                    return;
                }
            };

            let txn_fee = match txn_fee_input.value().parse::<u64>() {
                Ok(fee) => fee,
                Err(_) => {
                    error_display.show();
                    return;
                }
            };

            let meta = meta_input.value();

            let (mut txn, input_utxos, outputs) = match make_single_p2pkh_txn(dest_address, txn_amount, txn_fee, state) {
                Ok(data) => data,
                Err(err) => {
                    fltk::dialog::alert_default(&format!("Error: {}", err));
                    return;
                }
            };
            txn.meta = meta;

            let sig = match sign_txn(&txn, &state.keypair) {
                Ok(sig) => sig,
                Err(err) => {
                    fltk::dialog::alert_default(&format!("Error: {}", err));
                    return;
                }
            };

            let pubkey = state.keypair.public_key().as_ref().to_vec();
            let unlock_script = make_p2pkh_unlock(sig, pubkey);
            let txn_inputs = input_utxos
                .iter()
                .map(|c| TxnInput {
                    txn_hash: c.txn,
                    output_idx: c.output,
                    unlock_script: unlock_script.clone(),
                })
                .collect::<Vec<TxnInput>>();

            let unhashed = UnhashedTransaction {
                version: VERSION,
                inputs: txn_inputs,
                outputs,
                meta: txn.meta,
            };
        
            let hash = match hash_txn(&unhashed) {
                Ok(hash) => hash,
                Err(err) => {
                    fltk::dialog::alert_default(&format!("Error: {}", err));
                    return;
                }
            };
            let full_txn = unhashed.to_hashed(hash);

            match verify_transaction(full_txn.clone(), state) {
                Ok(_) => (),
                Err(err) => {
                    fltk::dialog::alert_default(&format!("Error: {}", err));
                    return;
                }
            };

            match send_new_txn(full_txn, state) {
                Ok(_) => {
                    println!("Sent transaction");
                },
                Err(_) => {
                    println!("Error sending transaction");
                }
            };

            win_clone.hide();
        });

        win.make_modal(true);
        win.end();

        Self { win }
    }
}
