use proc_macro::{Literal, TokenStream, TokenTree};

mod bitpack;
mod core_local;
mod fixutils;
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

#[proc_macro_derive(BitPack)]
pub fn bitpack(input: TokenStream) -> TokenStream {
    bitpack::bitpack(input)
}

#[proc_macro_attribute]
pub fn procedure_entrypoint(attr: TokenStream, item: TokenStream) -> TokenStream {
    fixutils::entrypoint(attr, item)
}

#[proc_macro]
pub fn num_fixutils_memories(_input: TokenStream) -> TokenStream {
    TokenTree::Literal(Literal::usize_unsuffixed(fixutils::NUM_MEMORIES)).into()
}

#[proc_macro]
pub fn num_fixutils_tables(_input: TokenStream) -> TokenStream {
    TokenTree::Literal(Literal::usize_unsuffixed(fixutils::NUM_TABLES)).into()
}
