use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

pub fn kmain(_: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    let ident = item.sig.ident.clone();
    quote! {
        #[no_mangle]
        extern "C" fn kmain(argc: usize, argv: *const usize) {
            #item

            let slice = unsafe {
                core::slice::from_raw_parts(argv, argc)
            };

            #ident(slice);

        }
    }
    .into()
}
