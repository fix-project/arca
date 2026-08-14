use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

pub fn entrypoint(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    let ident = &item.sig.ident;

    quote! {
        #item
        #[unsafe(export_name = "_fixpoint_apply_inner")]
        pub extern "C" fn _fixpoint_apply_inner(combination: ::fixutils::RustHandle) -> ::fixutils::RustHandle {
            let _fixpoint_apply: fn(::fixutils::RustHandle) -> ::fixutils::RustHandle = #ident;
            _fixpoint_apply(combination)
        }
    }
    .into()
}
