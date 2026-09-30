use proc_macro::TokenStream;
use quote::quote;
use std::{fmt::Write, ops::Range};
use syn::{ItemFn, parse_macro_input};

pub const NUM_MEMORIES: usize = 32;
pub const NUM_TABLES: usize = 32;

pub fn entrypoint(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    let _fixpoint_apply = &item.sig.ident;
    let asm = memory_asm() + &table_asm();

    quote! {
        // Declarations need to be included in procedure's object file during compilation for the inlined WebAssembly
        ::core::arch::global_asm!(::fixutils::declare_wasm!(), options(raw),);
        ::core::arch::global_asm!(#asm, options(raw));
        #item

        #[unsafe(export_name = "_fixpoint_apply_inner")]
        pub extern "C" fn _fixpoint_apply_inner() -> *const ::fixutils::HandleOp<'static> {
            let output = ::alloc::boxed::Box::new(
                #_fixpoint_apply(::fixutils::Combination)
                    .expect("expected _fixpoint_apply to succeed"),
            );

            // converts result output into a pointer for the webassembly _fixpoint_apply_wrapper
            ::alloc::boxed::Box::into_raw(output)
        }
    }
    .into()
}

fn write_asm(name: &str, signature: &str, indices: Range<usize>, body: &str) -> String {
    let index_local = usize::from(name == "wasm_table_set");
    let (start, count) = (indices.start, indices.len());
    let mut asm = format!(".globl {name}\n{name}:\n.functype {name} {signature}\n");
    // blocks for default fallback and each index
    for _ in 0..=count {
        asm.push_str("block\n");
    }
    writeln!(asm, "local.get {index_local}\ni32.const {start}\ni32.sub").unwrap();

    asm.push_str("br_table {");
    for depth in 0..count {
        write!(asm, "{depth}, ").unwrap();
    }
    write!(asm, "{count}").unwrap();
    asm.push_str("}\n");

    for index in indices {
        writeln!(
            asm,
            "end_block\n{}\nreturn",
            body.replace("{}", &index.to_string())
        )
        .unwrap();
    }
    asm.push_str("end_block\nunreachable\nend_function\n");
    asm
}

fn memory_asm() -> String {
    let mut asm = String::new();
    for (name, signature, body) in [
        (
            "wasm_memory_read",
            "(i32, i32, i32) -> ()",
            "local.get 1\ni32.const 0\nlocal.get 2\nmemory.copy 0, {}",
        ),
        (
            "wasm_memory_write",
            "(i32, i32, i32) -> ()",
            "i32.const 0\nlocal.get 1\nlocal.get 2\nmemory.copy {}, 0",
        ),
        ("wasm_memory_size", "(i32) -> (i32)", "memory.size {}"),
        (
            "wasm_memory_grow",
            "(i32, i32) -> (i32)",
            "local.get 1\nmemory.grow {}",
        ),
    ] {
        asm += &write_asm(name, signature, 1..NUM_MEMORIES + 1, body);
    }
    asm
}

fn table_asm() -> String {
    let mut asm = String::new();
    for index in 0..NUM_TABLES {
        asm += &format!(
            ".section .text.wasm_table_{index},\"\",@\n.globl wasm_table_{index}\n.tabletype wasm_table_{index}, externref\nwasm_table_{index}:\n"
        );
    }
    for (name, signature, body) in [
        (
            "wasm_table_get",
            "(i32, i32) -> (externref)",
            "local.get 1\ntable.get wasm_table_{}",
        ),
        (
            "wasm_table_set",
            "(externref, i32, i32) -> ()",
            "local.get 2\nlocal.get 0\ntable.set wasm_table_{}",
        ),
        (
            "wasm_table_size",
            "(i32) -> (i32)",
            "table.size wasm_table_{}",
        ),
        (
            "wasm_table_grow",
            "(i32, i32) -> (i32)",
            "ref.null_extern\nlocal.get 1\ntable.grow wasm_table_{}",
        ),
    ] {
        asm += &write_asm(name, signature, 0..NUM_TABLES, body);
    }
    asm
}
