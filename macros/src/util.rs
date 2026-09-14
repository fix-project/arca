use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

pub fn kmain(_: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    let ident = item.sig.ident.clone();
    quote! {
    /// # Safety
    /// There can only be one kmain in an executable.
        #[unsafe(no_mangle)]
        extern "C" fn kmain() {
            #item

            #ident();

        }
    }
    .into()
}
