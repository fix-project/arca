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
