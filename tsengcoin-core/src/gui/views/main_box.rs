use crate::gui::views::{new_chat::NewChatUI, BasicVisible};
use basic_visible_derive::BasicVisible;
use std::sync::{Arc, Mutex};

use fltk::{
    app::{channel, quit, Receiver},
    enums::Shortcut,
    menu::{MenuBar, MenuFlag},
    prelude::{GroupExt, MenuExt, WidgetExt},
    window::Window,
};

use crate::{
    gui::views::{new_alias::NewAliasUI, settings::SettingsUI},
    v1::state::State,
};

#[derive(Debug, Clone, BasicVisible)]
pub struct MainUI {
    pub win: Window,
    pub receiver: Receiver<MainUIMessage>,
}

#[derive(Copy, Clone, Debug)]
pub enum MainUIMessage {
    Settings,
    Quit,
    ViewAliases,
    NewAlias,
    NewChat,
    About,
}

impl MainUI {
    pub fn new() -> Self {
        let win = Window::default()
            .with_label("TsengCoin")
            .with_size(400, 300);
        let mut menu_bar = MenuBar::default().with_size(400, 20);

        let (sender, receiver) = channel();

        menu_bar.add_emit(
            "_File/_Settings\t",
            Shortcut::Ctrl | 's',
            MenuFlag::Normal,
            sender,
            MainUIMessage::Settings,
        );

        menu_bar.add_emit(
            "File/Quit\t",
            Shortcut::Ctrl | 'q',
            MenuFlag::Normal,
            sender,
            MainUIMessage::Quit,
        );

        menu_bar.add_emit(
            "_View/Aliases\t",
            Shortcut::None,
            MenuFlag::Normal,
            sender,
            MainUIMessage::ViewAliases,
        );

        menu_bar.add_emit(
            "_New/Alias\t",
            Shortcut::Ctrl | 'a',
            MenuFlag::Normal,
            sender,
            MainUIMessage::NewAlias,
        );

        menu_bar.add_emit(
            "New/Chat\t",
            Shortcut::None,
            MenuFlag::Normal,
            sender,
            MainUIMessage::NewChat,
        );

        menu_bar.add_emit(
            "_Help/About\t",
            Shortcut::None,
            MenuFlag::Normal,
            sender,
            MainUIMessage::About,
        );

        win.end();

        Self { win, receiver }
    }
}

pub fn handle_messages(state_arc: &Arc<Mutex<State>>, main_ui: &MainUI) {
    use MainUIMessage::*;
    if let Some(msg) = main_ui.receiver.recv() {
        match msg {
            Settings => {
                let mut settings = SettingsUI::new(Arc::clone(state_arc));
                settings.show();
            }
            Quit => {
                quit();
                // TODO: Quitting logic
            }
            ViewAliases => {
                println!("view aliases")
            }
            NewAlias => {
                let mut new_alias = NewAliasUI::new(Arc::clone(state_arc));
                new_alias.show();
            }
            NewChat => {
                let mut new_chat = NewChatUI::new(Arc::clone(state_arc));
                new_chat.show();
            }
            About => {
                fltk::dialog::message_default("TsengCoin core client, written in Rust. GUI built with FLTK (Fast Light Toolkit).\nSource code at https://github.com/Dezzmeister/tsengcoin");
            }
        }
    }
}
