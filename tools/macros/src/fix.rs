use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

pub fn apply(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    let callback = &item.sig.ident;
    quote! {
        #item
        #[unsafe(export_name = "_fix_apply_inner")]
        pub extern "C" fn __fix_apply() -> u32 {
            match ::fix::__apply(|combination| ::fix::__finish(#callback(combination)?)) {
                Ok(()) => 0,
                Err(error) => error as u32,
            }
        }
    }
    .into()
}
