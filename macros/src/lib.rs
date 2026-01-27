use proc_macro::TokenStream;

mod core_local;
mod testing;
mod util;

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

#[proc_macro_attribute]
pub fn profile(attr: TokenStream, item: TokenStream) -> TokenStream {
    testing::profile(attr, item)
}

#[proc_macro_attribute]
pub fn arca_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn kmain(attr: TokenStream, item: TokenStream) -> TokenStream {
    util::kmain(attr, item)
}
