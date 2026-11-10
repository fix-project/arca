#![no_std]
#![allow(unused)]
#![feature(slice_from_ptr_range)]
#![feature(atomic_ptr_null)]
#![feature(cstr_display)]

extern crate alloc;

use alloc::alloc::{Layout, alloc_zeroed, handle_alloc_error};
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
    rt::{wasm_rt_init, wasm_rt_module_alignment, wasm_rt_module_size},
};

mod abi;
mod continuation;
mod descriptors;
mod fixpoint;
mod handles;
mod locals;
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

unsafe extern "C" {
    static mut _sbss: c_void;
    static mut _ebss: c_void;
    fn wasm_rt_module_instantiate(module: *mut c_void, combination: *const w2c_fix);
    fn wasm2c_module_free(module: *mut c_void);
    fn w2c_module_0x5Ffix_apply(module: *const c_void, combination: u32) -> u32;
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
    let handle = continuation::read_reply(os::argument());
    let result = unsafe {
        let argument = descriptors::initialize(handle);
        wasm_rt_init();
        let layout =
            Layout::from_size_align(wasm_rt_module_size(), wasm_rt_module_alignment()).unwrap();
        let module = alloc_zeroed(layout).cast::<c_void>();
        if module.is_null() {
            handle_alloc_error(layout);
        }

        wasm_rt_module_instantiate(module, core::ptr::null());

        let result = w2c_module_0x5Ffix_apply(module, argument);
        descriptors::address(result)
    };
    continuation::finish(result);
}
