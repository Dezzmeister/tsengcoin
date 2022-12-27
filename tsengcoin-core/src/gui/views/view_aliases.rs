use std::sync::{Arc, Mutex};

use fltk::{
    button::{ReturnButton},
    enums::{LabelType},
    prelude::{WidgetBase, WidgetExt, WindowExt, GroupExt, TableExt},
    window::Window,
};
use fltk_table::{SmartTable, TableOpts};

use crate::{
    gui::views::BasicVisible,
    v1::{
        state::State,
    }, wallet::{Address, address_to_b58c},
};
use basic_visible_derive::BasicVisible;

#[derive(BasicVisible)]
pub struct ViewAliasesUI {
    pub win: Window,
}

impl ViewAliasesUI {
    pub fn new(state_arc: Arc<Mutex<State>>) -> Self {
        let aliases = {
            let state = &state_arc.lock().unwrap();

            state.friends.aliases
                .iter()
                .map(|(addr, alias)| (addr.to_owned(), alias.to_owned()))
                .collect::<Vec<(Address, String)>>()
        };

        let mut win = Window::default()
            .with_size(500, 600)
            .with_label("View Aliases");

        let mut close_btn = ReturnButton::new(425, 570, 66, 20, "Close");

        let mut table = SmartTable::new(20, 20, 460, 540, "")
            .with_opts(TableOpts {
                rows: aliases.len().try_into().unwrap(),
                cols: 2,
                editable: false,
                ..Default::default()
            });

        table.set_label_type(LabelType::None);

        table.set_col_header_value(0, "Alias");
        table.set_col_width(0, 100);
        table.set_col_resize_min(100);

        table.set_col_header_value(1, "Address");
        table.set_col_width(1, 360);
        table.set_col_resize(false);

        table.set_row_header(false);

        for i in 0..aliases.len() {
            let row: i32 = i.try_into().unwrap();
            let (addr, alias) = &aliases[i];
            let addr_str = address_to_b58c(&addr.to_vec());

            table.set_cell_value(row, 0, alias);
            table.set_cell_value(row, 1, &addr_str);
        }

        table.resize_callback(|table, _, _, w, _| {
            // Address column should always have the same size to make resizing simpler
            let address_width = table.col_width(1);

            table.set_col_width(0, w - address_width);
        });

        let mut win_clone = win.clone();

        close_btn.set_callback(move |_| {
            win_clone.hide();
        });

        win.make_resizable(true);
        win.make_modal(true);
        win.end();

        Self { win }
    }
}
