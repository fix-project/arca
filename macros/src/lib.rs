use proc_macro::TokenStream;

mod core_local;
mod testing;

#[proc_macro_attribute]
pub fn core_local(attr: TokenStream, item: TokenStream) -> TokenStream {
    core_local::body(attr, item)
}

#[proc_macro_attribute]
pub fn test(attr: TokenStream, item: TokenStream) -> TokenStream {
    testing::test(attr, item)
}

#[proc_macro_attribute]
pub fn bench(attr: TokenStream, item: TokenStream) -> TokenStream {
    testing::bench(attr, item)
}
