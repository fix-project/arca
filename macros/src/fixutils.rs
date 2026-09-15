use proc_macro::TokenStream;
use quote::quote;
use std::fmt::Write;
use syn::{ItemFn, LitInt, parse_macro_input};

pub fn entrypoint(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    let _fixpoint_apply = &item.sig.ident;

    quote! {
        // Declarations need to be included in procedure's object file during compilation for the inlined WebAssembly
        ::core::arch::global_asm!(::fixutils::declare_wasm!(), options(raw),);
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

fn write_asm(name: &str, signature: &str, count: usize, body: &str) -> String {
    let index_local = usize::from(name == "wasm_table_set");
    let mut asm = format!(".globl {name}\n{name}:\n.functype {name} {signature}\n");
    // blocks for default fallback and index 1 to count
    for _ in 0..=count {
        asm.push_str("block\n");
    }
    writeln!(asm, "local.get {index_local}\ni32.const 1\ni32.sub").unwrap();

    asm.push_str("br_table {");
    for depth in 0..count {
        write!(asm, "{depth}, ").unwrap();
    }
    write!(asm, "{count}").unwrap();
    asm.push_str("}\n");

    for index in 1..=count {
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

fn memory_asm(count: usize) -> String {
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
        asm += &write_asm(name, signature, count, body);
    }
    // Number of memories encoded in custom section
    asm + &format!(".section .custom_section.wasm_num_memories,\"\",@\n.int32 {count}\n")
}

fn table_asm(count: usize) -> String {
    let mut asm = String::new();
    for index in 1..=count {
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
        asm += &write_asm(name, signature, count, body);
    }
    asm
}

pub fn num_memories(input: TokenStream) -> TokenStream {
    let count: usize = match parse_macro_input!(input as LitInt).base10_parse() {
        Ok(count) => count,
        Err(error) => return error.to_compile_error().into(),
    };
    let asm = memory_asm(count);
    quote! {
        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub static UTIL_NUM_MEMORIES: u32 = #count as u32;

        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub extern "C" fn util_allocate_memory(index: u32) -> *mut ::fixutils::Memory {
            use ::core::sync::atomic::{AtomicBool, Ordering};

            const COUNT: usize = #count;
            static mut SLOTS: [::fixutils::Memory; COUNT] = [const { ::fixutils::Memory::EMPTY }; COUNT];
            static OCCUPIED: [AtomicBool; COUNT] = [const { AtomicBool::new(false) }; COUNT];

            // can't get memory 0, memory above count, or already occupied memory
            if index == 0 || index as usize > COUNT {
                return ::core::ptr::null_mut();
            }
            let slot_index = index as usize - 1;
            if OCCUPIED[slot_index].swap(true, Ordering::Relaxed) {
                return ::core::ptr::null_mut();
            }
            unsafe { (&raw mut SLOTS).cast::<::fixutils::Memory>().add(slot_index) }
        }

        ::core::arch::global_asm!(#asm, options(raw));
    }
    .into()
}

pub fn num_tables(input: TokenStream) -> TokenStream {
    let count: usize = match parse_macro_input!(input as LitInt).base10_parse() {
        Ok(count) => count,
        Err(error) => return error.to_compile_error().into(),
    };
    let asm = table_asm(count);
    quote! {
        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub static UTIL_NUM_TABLES: u32 = #count as u32;

        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub extern "C" fn util_allocate_table(index: u32) -> *mut ::fixutils::Table {
            use ::core::sync::atomic::{AtomicBool, Ordering};

            const COUNT: usize = #count;
            static mut SLOTS: [::fixutils::Table; COUNT] = [const { ::fixutils::Table::EMPTY }; COUNT];
            static OCCUPIED: [AtomicBool; COUNT] = [const { AtomicBool::new(false) }; COUNT];

            // can't get table 0, table above count, or already occupied table
            if index == 0 || index as usize > COUNT {
                return ::core::ptr::null_mut();
            }
            let slot_index = index as usize - 1;
            if OCCUPIED[slot_index].swap(true, Ordering::Relaxed) {
                return ::core::ptr::null_mut();
            }
            unsafe { (&raw mut SLOTS).cast::<::fixutils::Table>().add(slot_index) }
        }

        ::core::arch::global_asm!(#asm, options(raw));
    }
    .into()
}
