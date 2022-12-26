use crate::gui::views::{new_chat::NewChatUI, BasicVisible};
use basic_visible_derive::BasicVisible;
use std::sync::{Arc, Mutex};

use fltk::{
    app::{channel, quit, Receiver},
    enums::{Shortcut, LabelType, Color},
    menu::{MenuBar, MenuFlag},
    prelude::{GroupExt, MenuExt, WidgetExt, ImageExt, WidgetBase, InputExt},
    window::Window, image::PngImage, frame::Frame, output::Output,
};

use crate::{
    gui::views::{new_alias::NewAliasUI, settings::SettingsUI},
    v1::state::State,
};

const LOGO: &[u8] = include_bytes!("../../../assets/logo.png");

#[derive(Debug, Clone, BasicVisible)]
pub struct MainUI {
    pub win: Window,
    pub receiver: Receiver<MainUIMessage>,
    pub address_view: Output,
    pub balance_view: Output
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
    pub fn new(address: &str) -> Self {
        let win = Window::default()
            .with_label("TsengCoin")
            .with_size(400, 400);

        let mut frm = Frame::new(0, 0, 400, 400, "");

        if let Ok(mut logo_bg) = PngImage::from_data(LOGO) {
            logo_bg.scale(400, 400, true, false);
            frm.set_image(Some(logo_bg));
        }

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

        let mut address_view = Output::new(0, 378, 275, 22, "");
        address_view.set_label_type(LabelType::None);
        address_view.set_text_size(10);
        address_view.set_color(Color::from_hex(0xc0c0c0));
        address_view.set_tooltip("Your Address");
        address_view.set_value(address);

        let mut balance_view = Output::new(275, 378, 125, 22, "");
        balance_view.set_label_type(LabelType::None);
        balance_view.set_text_size(10);
        balance_view.set_color(Color::from_hex(0xc0c0c0));
        balance_view.set_tooltip("Your Balance (TGC)");
        balance_view.set_value("...");

        win.end();

        Self { win, receiver, address_view, balance_view }
    }

    pub fn set_balance(&mut self, balance: u64) {
        self.balance_view.set_value(&format!("{} TGC", balance));
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
