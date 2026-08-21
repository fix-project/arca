use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{parse_macro_input, ItemFn, LitInt};

pub fn entrypoint(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    let _fixpoint_apply = &item.sig.ident;

    quote! {
        #item
        #[unsafe(export_name = "_fixpoint_apply_inner")]
        pub extern "C" fn _fixpoint_apply_inner(combination: ::fixutils::RustHandle<'static>) -> ::fixutils::RustHandle<'static> {
            #_fixpoint_apply(combination)
        }
    }
    .into()
}

fn registry(count: usize, kind: &str, lookup: &str) -> TokenStream2 {
    let kind = format_ident!("{kind}");
    let lookup = format_ident!("{lookup}");

    quote! {
        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub extern "C" fn #lookup(index: u16) -> *mut ::fixutils::#kind {
            use ::core::sync::atomic::{AtomicBool, Ordering};

            const COUNT: usize = #count;
            static mut SLOTS: [::fixutils::#kind; COUNT] = [const { ::fixutils::#kind::EMPTY }; COUNT];
            static OCCUPIED: [AtomicBool; COUNT] = [const { AtomicBool::new(false) }; COUNT];
            let slot_index = index as usize - 1;

            // can't get memory 0, memory above count, or already occupied memory
            if index == 0 || index as usize > COUNT || OCCUPIED[slot_index].swap(true, Ordering::Relaxed) {
                return ::core::ptr::null_mut();
            }
            unsafe { (&raw mut SLOTS).cast::<::fixutils::#kind>().add(slot_index) }
        }
    }
}

fn memory_asm(count: usize) -> String {
    let mut asm = String::new();
    for (name, signature, body) in [
        (
            "fix_memory_read",
            "(i32, i32, i32) -> ()",
            "local.get 1\ni32.const 0\nlocal.get 2\nmemory.copy 0, {}",
        ),
        (
            "fix_memory_write",
            "(i32, i32, i32) -> ()",
            "i32.const 0\nlocal.get 1\nlocal.get 2\nmemory.copy {}, 0",
        ),
        ("fix_memory_size", "(i32) -> (i32)", "memory.size {}"),
        (
            "fix_memory_grow",
            "(i32, i32) -> (i32)",
            "local.get 1\nmemory.grow {}",
        ),
    ] {
        asm += &format!(
            ".section .text.{name},\"\",@\n.globl {name}\n{name}:\n.functype {name} {signature}\n"
        );
        // Match statement
        for index in 1..=count {
            let body = body.replace("{}", &index.to_string());
            asm += &format!("local.get 0\ni32.const {index}\ni32.eq\nif\n{body}\nreturn\nend_if\n");
        }
        asm += "unreachable\nend_function\n";
    }
    // Number of memories encoded in custom section
    asm + &format!(".section .custom_section.num_fix_memories,\"\",@\n.int32 {count}\n")
}

fn table_asm(count: usize) -> String {
    let mut asm = String::new();
    // Tables
    for index in 1..=count {
        asm += &format!(
            ".section .text.fix_table_{index},\"\",@\n.globl fix_table_{index}\n.tabletype fix_table_{index}, externref\nfix_table_{index}:\n"
        );
    }
    for (name, signature, body) in [
        (
            "fixpoint_table_get",
            "(i32, i32) -> (externref)",
            "local.get 1\ntable.get fix_table_{}",
        ),
        (
            "fixpoint_table_set",
            "(i32, i32, externref) -> ()",
            "local.get 1\nlocal.get 2\ntable.set fix_table_{}",
        ),
        (
            "fix_table_size",
            "(i32) -> (i32)",
            "table.size fix_table_{}",
        ),
        (
            "fix_table_grow",
            "(i32, i32) -> (i32)",
            "ref.null_extern\nlocal.get 1\ntable.grow fix_table_{}",
        ),
    ] {
        asm += &format!(
            ".section .text.{name},\"\",@\n.globl {name}\n{name}:\n.functype {name} {signature}\n"
        );
        // Match statement
        for index in 1..=count {
            let body = body.replace("{}", &index.to_string());
            asm += &format!("local.get 0\ni32.const {index}\ni32.eq\nif\n{body}\nreturn\nend_if\n");
        }
        asm += "unreachable\nend_function\n";
    }
    asm
}

pub fn num_memories(input: TokenStream) -> TokenStream {
    let count: usize = match parse_macro_input!(input as LitInt).base10_parse() {
        Ok(count) => count,
        Err(error) => return error.to_compile_error().into(),
    };
    let registry = registry(count, "Memory", "fix_memory_slot");
    let asm = memory_asm(count);
    quote! { #registry ::core::arch::global_asm!(#asm); }.into()
}

pub fn num_tables(input: TokenStream) -> TokenStream {
    let count: usize = match parse_macro_input!(input as LitInt).base10_parse() {
        Ok(count) => count,
        Err(error) => return error.to_compile_error().into(),
    };
    let registry = registry(count, "Table", "fix_table_slot");
    let asm = table_asm(count);
    quote! { #registry ::core::arch::global_asm!(#asm); }.into()
}
