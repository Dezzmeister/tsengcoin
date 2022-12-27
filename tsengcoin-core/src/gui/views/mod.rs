pub mod chat_box;
pub mod main_box;
pub mod new_alias;
pub mod new_chat;
pub mod settings;
pub mod new_transaction;
pub mod txn_builder;
pub mod view_utxos;
pub mod new_unlock_script;

pub trait BasicVisible {
    fn show(&mut self);

    fn hide(&mut self);

    fn shown(&self) -> bool;
}
