#![no_std]
#![allow(unused)]
#![feature(slice_from_ptr_range)]
#![feature(atomic_ptr_null)]

use core::{
    arch::{asm, global_asm},
    ffi::c_void,
    ops::Range,
};

use user::{error, os, prelude::*};

use crate::{
    fixpoint::w2c_fixpoint,
    rt::{wasm_rt_externref_t, wasm_rt_free, wasm_rt_init, wasm_rt_module_size},
};

mod fixpoint;
mod rt;
mod runtime;
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
    fn wasm2c_module_instantiate(module: *mut c_void, combination: *const w2c_fixpoint);
    fn wasm2c_module_free(module: *mut c_void);
    fn w2c_module_0x5Ffixpoint_apply(
        module: *const c_void,
        combination: wasm_rt_externref_t,
    ) -> wasm_rt_externref_t;
}

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
    let combination = os::argument();
    let combination =
        Blob::try_from(combination).expect("fix programs must receive a handle as input");
    let mut handle = [0; 32];
    combination.read(0, &mut handle);
    let result = unsafe {
        wasm_rt_init();
        let module_size = wasm_rt_module_size();
        let result = alloca::with_alloca_zeroed(module_size, |module_buf| {
            let module = &raw mut module_buf[0] as *mut c_void;
            wasm2c_module_instantiate(module, core::ptr::null());
            let wasm_rt_externref_t { bytes: result } =
                w2c_module_0x5Ffixpoint_apply(module, wasm_rt_externref_t { bytes: handle });
            result
        });
        wasm_rt_free();
        result
    };
    os::exit(&result[..]);
}
