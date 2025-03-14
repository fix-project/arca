use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::{parse_macro_input, Ident, ItemFn};

fn kernel_ident() -> Ident {
    let found_crate = crate_name("kernel").expect("kernel is present in `Cargo.toml`");

    match found_crate {
        FoundCrate::Itself => format_ident!("crate"),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    }
}

pub fn kmain(_: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    let ident = item.sig.ident.clone();
    let kernel = kernel_ident();
    quote! {
        #[no_mangle]
        extern "C" fn kmain() {
            #item

            #kernel::rt::spawn(async {
                #ident().await;
            });
        }
    }
    .into()
}
