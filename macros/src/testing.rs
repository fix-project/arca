use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::{parse_macro_input, Ident, ItemFn};

#[cfg(feature = "testing-mode")]
use {
    quote::ToTokens,
    syn::{Attribute, Item, ItemMod},
};

fn kernel_ident() -> Ident {
    let found_crate = crate_name("kernel").expect("kernel is present in `Cargo.toml`");

    match found_crate {
        FoundCrate::Itself => format_ident!("crate"),
        FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
    }
}

pub fn test(_: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    let mut inner = item.clone();
    let ident = item.sig.ident;
    inner.sig.ident = format_ident!("test_case");
    let kernel = kernel_ident();
    quote! {
        #[test_case]
        fn #ident() {
            #inner

            use #kernel::debugcon::CONSOLE;
            use core::fmt::Write;
            let mut console = CONSOLE.lock();
            write!(console, "Test {}::{}: ", module_path!(), stringify!(#ident)).unwrap();
            console.unlock();
            test_case();
            let mut console = CONSOLE.lock();
            write!(console, "PASS\n").unwrap();
            console.unlock();
        }
    }
    .into()
}

pub fn bench(_: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    let mut inner = item.clone();
    let ident = item.sig.ident;
    inner.sig.ident = format_ident!("test_case");
    let kernel = kernel_ident();
    let iters: usize = 0x80;
    let reps: usize = 0x20;
    quote! {
        #[test_case]
        fn #ident() {
            #inner
            test_case(|f| {
                use core::time::Duration;
                use alloc::vec;
                use #kernel::debugcon::CONSOLE;
                use core::fmt::Write;
                let mut console = CONSOLE.lock();
                write!(console, "Test {}::{}: ", module_path!(), stringify!(#ident)).unwrap();
                console.unlock();

                let mut ts = vec![0.0; #reps];
                for t in ts.iter_mut() {
                    *t = #kernel::tsc::time(|| {
                        for _ in 0..#iters {
                            f();
                        }
                    }).as_secs_f64() / (#iters as f64);
                }

                ts.sort_by(|a, b| a.partial_cmp(b).unwrap());

                let median = (ts[ts.len()/2] + ts[(ts.len() + 1) / 2])/2.0;
                let deviation = ts[ts.len() - 1] - ts[0];

                let median = (median * 1e9) as u64;
                let deviation = (deviation * 1e9) as u64;

                let mut console = CONSOLE.lock();
                write!(console, "{} ns/iter (+/- {})\n", median, deviation).unwrap();
                console.unlock();
            });
        }
    }
    .into()
}

pub fn profile(_: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = item;
    let kernel = kernel_ident();
    let name = sig.ident.to_string();
    quote! {
        #(#attrs)*
        #vis #sig {
            static __function_name: &str = const {
                concat!(#name, "@", file!(), ":", line!())
            };
            let mut f = async || #block;

            let __start = #kernel::kvmclock::time_since_boot();
            let result = f().await;
            let end = #kernel::kvmclock::time_since_boot();
            let duration = end - __start;
            #kernel::aprofile::log_time_spent(&__function_name, duration);
            result
        }
    }
    .into()
}

#[cfg_attr(not(feature = "testing-mode"), allow(unused))]
pub fn inline_module_test(_: TokenStream, item: TokenStream) -> TokenStream {
    #[cfg(feature = "testing-mode")]
    {
        let mut module = parse_macro_input!(item as ItemMod);

        let Some((_, items)) = &mut module.content else {
            panic!("inline_module_test: module must be inline (use `mod m {{ }} or `include!()`")
        };

        fn has_attr(attrs: &[Attribute], name: &str) -> bool {
            attrs.iter().any(|a| a.path().is_ident(name))
        }

        fn generate_desc(item: &ItemFn, test_descs: &mut Vec<Item>, tests: &mut Vec<Ident>) {
            if has_attr(&item.attrs, "arca_test") {
                let test_name = item.sig.ident.clone();
                let test_obj_name = format_ident!("{}_obj", item.sig.ident);
                let test_desc = quote! {
                    #[allow(non_upper_case_globals)]
                    const #test_obj_name: TestDescAndFn = TestDescAndFn {
                        name: stringify!(#test_name),
                        function: #test_name
                    };
                };

                test_descs
                    .push(syn::parse::<Item>(test_desc.into()).expect("Failed to parse test"));
                tests.push(test_obj_name);
            }
        }

        let use_clause = quote! {
            use crate::{TestDescAndFn, ModuleDesc, vec, arca_test};
        };
        let mut tests = vec![];
        let mut test_descs = vec![];

        for it in items.as_slice() {
            if let Item::Fn(func) = it {
                generate_desc(func, &mut test_descs, &mut tests)
            }
        }

        let module_name = module.ident.clone();

        let module_desc = quote! {
            pub const __MODULE_TESTS: ModuleDesc = ModuleDesc {
                name: stringify!(#module_name),
                functions: &[ #( #tests ),* ]
            };
        };

        items.push(syn::parse::<Item>(use_clause.into()).unwrap());
        items.extend(test_descs);
        items.push(syn::parse(module_desc.into()).unwrap());
        module.into_token_stream().into()
    }

    #[cfg(not(feature = "testing-mode"))]
    {
        quote! {}.into()
    }
}

#[cfg_attr(not(feature = "testing-mode"), allow(unused))]
pub fn arca_module_test(_: TokenStream, item: TokenStream) -> TokenStream {
    #[cfg(feature = "testing-mode")]
    {
        let module = parse_macro_input!(item as ItemMod);
        let module_name = module.ident.clone();

        match &module.content {
            Some(_) => {
                let module_content = module.into_token_stream();
                quote! {
                    #[inline_module_test]
                    #module_content
                }
                .into()
            }
            None => {
                use std::env;
                use std::fs;
                use std::path::PathBuf;

                let mut src_dir: PathBuf = env::var("CARGO_MANIFEST_DIR").unwrap().into();
                src_dir.push("src");
                src_dir.push(module_name.to_string() + ".rs");
                let file_content: proc_macro2::TokenStream = fs::read_to_string(src_dir)
                    .expect("arca_module_test: failed to load src file")
                    .parse()
                    .expect("arca_module_test: Failed to parse");
                quote! {
                    #[inline_module_test]
                    mod #module_name {
                        #file_content
                    }
                }
                .into()
            }
        }
    }

    #[cfg(not(feature = "testing-mode"))]
    {
        quote! {}.into()
    }
}
