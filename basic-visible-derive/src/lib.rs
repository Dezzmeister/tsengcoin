extern crate proc_macro;
extern crate syn;

#[macro_use]
extern crate quote;

use proc_macro::TokenStream;

#[proc_macro_derive(BasicVisible)]
pub fn basic_visible_derive(tokens: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(tokens as syn::DeriveInput);

    match input.data {
        syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Named(fields),
            ..
        }) => {
            let has_win = fields.named
                .iter()
                .any(|f| f.ident.as_ref().unwrap().clone() == "win");

            if !has_win {
                panic!("Struct needs to have a field named 'win' with the type 'Window' in order to be annotated with BasicVisible");
            }
        },
        _ => panic!("#[derive(BasicVisible)] is only defined for structs")
    };

    let name = &input.ident;

    let code = quote!{
        impl BasicVisible for #name {
            fn show(&mut self) {
                self.win.show();
            }

            fn hide(&mut self) {
                self.win.hide();
            }

            fn shown(&self) -> bool {
                use fltk::prelude::WindowExt;
                self.win.shown()
            }
        }
    };

    code.into()
}
