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

            use #kernel::debugcon::DebugConsole;
            use core::fmt::Write;
            write!(DebugConsole, "Test {}::{}: ", module_path!(), stringify!(#ident)).unwrap();
            test_case();
            write!(DebugConsole, "PASS\n").unwrap();
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
                use #kernel::debugcon::DebugConsole;
                use core::fmt::Write;
                write!(DebugConsole, "Test {}::{}: ", module_path!(), stringify!(#ident)).unwrap();

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

                write!(DebugConsole, "{} ns/iter (+/- {})\n", median, deviation).unwrap();
            });
        }
    }
    .into()
}
