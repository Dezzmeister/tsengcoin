pub mod chat_box;
pub mod main_box;
pub mod settings;
pub mod new_alias;
pub mod new_chat;

pub trait BasicVisible {
    fn show(&mut self);

    fn hide(&mut self);

    fn shown(&self) -> bool;
}
