#![no_std]
#![feature(portable_simd, simd_ffi)]
#![allow(unused)]
#![feature(slice_from_ptr_range)]
#![feature(atomic_ptr_null)]
#![feature(cstr_display)]

extern crate alloc;

use core::{
    arch::{asm, global_asm},
    ffi::c_void,
    ops::Range,
};

use user::Runtime;
use user::error::log as arca_log;
use user::{error, os, prelude::*};

use crate::{
    fixpoint::w2c_fix,
    rt::{
        wasm_rt_externref_t, wasm_rt_free, wasm_rt_init, wasm_rt_module_alignment,
        wasm_rt_module_size,
    },
};

mod exceptions;
mod fixpoint;
mod rt;
pub mod shell;

global_asm!(
    r#"
.section .text.start
.extern _rsstart
.extern __stack_top
.globl _start
_start:
  lea rsp, __stack_top[rip]
  call _rsstart
.halt:
  int3
  jmp .halt
.globl bail
bail:
  mov rdi, 0
  mov rax, 3
  syscall
  int3
.section .text
"#
);

pub static mut _PROCEDURE: core::simd::u8x32 = core::simd::u8x32::splat(0);

#[allow(
    improper_ctypes,
    reason = "userspace uses the System V SIMD ABI; vector layout is asserted in rt"
)]
unsafe extern "C" {
    static mut _sbss: c_void;
    static mut _ebss: c_void;
    fn wasm2c_module_instantiate(module: *mut c_void, combination: *const w2c_fix);
    fn wasm2c_module_free(module: *mut c_void);
    fn w2c_module_0x5Ffix_apply(
        module: *const c_void,
        combination: wasm_rt_externref_t,
    ) -> wasm_rt_externref_t;
}

/// The Rust entrypoint for Fix-on-Arca programs.  This function performs the minimal runtime setup
/// to ensure other Rust code can run correctly.
///
/// # Safety
///
/// This function must be called exactly once, before any other Rust code has run.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _rsstart() -> ! {
    unsafe {
        let bss = core::slice::from_mut_ptr_range(Range {
            start: &raw mut _sbss as *mut u8,
            end: &raw mut _ebss as *mut u8,
        });

        bss.fill(0);
    }

    main();
}

// Size in bytes of the buffer for the wasm2c module instance (w2c_module). Must be at least wasm_rt_module_size()
const MODULE_BUF_SIZE: usize = 8192;
#[repr(C, align(32))]
struct ModuleBuffer([u8; MODULE_BUF_SIZE]);

static mut MODULE_BUF: ModuleBuffer = ModuleBuffer([0; MODULE_BUF_SIZE]);

pub fn main() -> ! {
    unsafe extern "C" {
        static __stack_bottom: u8;
    }
    assert_eq!(
        unsafe {
            arcane::arca_mprotect(
                (&raw const __stack_bottom).cast_mut().cast(),
                arcane::__MODE_none as i32,
            )
        },
        0
    );
    let combination = os::argument();
    let combination =
        Blob::try_from(combination).expect("fix programs must receive a handle as input");
    let mut handle = [0; 32];
    assert_eq!(combination.len(), handle.len());
    assert_eq!(combination.read(0, &mut handle), handle.len());
    let result = unsafe {
        wasm_rt_init();
        let module_size = wasm_rt_module_size();
        let module = unsafe {
            assert!(module_size <= MODULE_BUF_SIZE);
            &raw mut MODULE_BUF.0[0] as *mut c_void
        };
        assert_eq!(module as usize % wasm_rt_module_alignment(), 0);

        /// Read procedure handle from combination
        {
            let tree = Function::symbolic("get_tree")
                .apply(Blob::new(handle))
                .call_with_current_continuation();

            let procedure_ref = &raw mut _PROCEDURE;
            assert!(shell::fixpoint_len(core::simd::u8x32::from_array(handle)) >= 1);
            shell::read_backing(&tree, 0, (&mut *procedure_ref).as_mut_array());
        }

        wasm2c_module_instantiate(module, core::ptr::null());

        let wasm_rt_externref_t { bytes: result } = w2c_module_0x5Ffix_apply(
            module,
            wasm_rt_externref_t {
                bytes: core::simd::u8x32::from_array(handle),
            },
        );
        wasm_rt_free();
        result
    };
    os::exit(result.as_array().as_slice());
}
