use std::sync::{Arc, Mutex};

use fltk::{
    button::Button,
    enums::{Align},
    input::{Input, IntInput},
    prelude::{DisplayExt, GroupExt, InputExt, WidgetBase, WidgetExt, WindowExt},
    text::{TextEditor, TextBuffer},
    window::Window,
};

use crate::{gui::views::BasicVisible, v1::{state::State, transaction::{ClaimedUTXO, UTXOWindow}}};
use basic_visible_derive::BasicVisible;

const HELP_TXT: &str = 
"Use this form to unlock transaction outputs with custom lock scripts. Use the variables 
`<sig>` and `<pubkey>`in your unlock script to refer to your public key and the signature
of a future transaction. These variables will be substituted when you use this unlock script to
spend the output.";

#[derive(BasicVisible)]
pub struct NewUnlockScriptUI {
    pub win: Window,
}

impl NewUnlockScriptUI {
    pub fn new(state_arc: Arc<Mutex<State>>) -> Self {
        let mut win = Window::default()
            .with_size(400, 266)
            .with_label("New Unlock Script");
        let mut save_btn = Button::new(325, 236, 64, 20, "Save");
        let mut cancel_btn = Button::new(255, 236, 64, 20, "Cancel");
        let mut help_btn = Button::new(11, 236, 64, 20, "Help");
        let mut txn_hash_input = Input::new(20, 32, 360, 22, "Transaction Hash");
        txn_hash_input.set_align(Align::TopLeft);

        let mut output_idx_input = IntInput::new(20, 82, 80, 22, "Output Index");
        output_idx_input.set_align(Align::TopLeft);

        let mut script_input = TextEditor::new(20, 132, 360, 88, "Unlock Script");
        script_input.set_align(Align::TopLeft);

        script_input.set_buffer(TextBuffer::default());

        let btn_state_arc = Arc::clone(&state_arc);
        let mut win_clone = win.clone();
        let mut win_clone_2 = win.clone();

        cancel_btn.set_callback(move |_| {
            win_clone_2.hide();
        });

        help_btn.set_callback(move |_| {
            fltk::dialog::message_default(HELP_TXT);
        });

        save_btn.set_callback(move |_| {
            let state_mut = &btn_state_arc;
            let state = &mut state_mut.lock().unwrap();

            let txn_hash_vec = match hex::decode(txn_hash_input.value()) {
                Ok(hash) => hash,
                Err(err) => {
                    fltk::dialog::alert_default(&format!("Transaction hash error: {}", err));
                    return;
                }
            };

            let mut txn_hash = [0_u8; 32];
            txn_hash[(32 - txn_hash_vec.len())..].copy_from_slice(&txn_hash_vec);

            let output_idx = match output_idx_input.value().parse::<usize>() {
                Ok(idx) => idx,
                Err(_) => {
                    fltk::dialog::alert_default("Invalid output index");
                    return;
                }
            };

            let unlock_script_code = match script_input.buffer() {
                Some(buf) if !buf.text().is_empty() => buf.text(),
                _ => {
                    fltk::dialog::alert_default("Provide an unlock script");
                    return;
                }
            };

            let txn_idx = match state.blockchain.utxo_pool.find_txn_index(txn_hash) {
                Some(txn_idx) => txn_idx,
                None => {
                    fltk::dialog::alert_default("Transaction doesn't exist");
                    return;
                }
            };

            if !txn_idx.outputs.contains(&output_idx) {
                fltk::dialog::alert_default("Invalid output index");
                return;
            }

            let txn = match txn_idx.lookup_txn(state) {
                Some(txn) => txn,
                None => {
                    fltk::dialog::alert_default("Unknown error");
                    return;
                }
            };

            let amount = txn.outputs[output_idx].amount;

            let claimed_utxo = ClaimedUTXO {
                window: UTXOWindow {
                    block: txn_idx.block,
                    txn: txn_idx.txn,
                    output: output_idx,
                    amount,
                },
                unlock_script: unlock_script_code
            };

            match state.claim_utxo(claimed_utxo) {
                Ok(()) => (),
                Err(err) => {
                    fltk::dialog::alert_default(&format!("Error: {}", err));
                    return;
                }
            };

            win_clone.hide();
        });

        win.make_modal(true);
        win.end();

        Self { win }
    }
}
