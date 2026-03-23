#![no_std]
#![allow(unused)]
#![feature(slice_from_ptr_range)]

use core::{
    arch::{asm, global_asm},
    ffi::c_void,
    ops::Range,
};

use user::{error, os, prelude::*};

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
  mov rbx, 0
  call _rsstart
.halt:
  int3
  jmp .halt
.section .text
"#
);

#[repr(C)]
pub struct ExternRef(pub [u8; 32]);

unsafe extern "C" {
    static mut _sbss: c_void;
    static mut _ebss: c_void;
    fn w2c_module_0x5Ffixpoint_apply(module: *const c_void, combination: ExternRef);
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
    let handle = os::argument();
    error::log("within the fix shell");
    // unsafe {
    //     w2c_module_0x5Ffixpoint_apply(todo!(), todo!());
    // }
    os::exit(handle);
}

#[repr(C)]
pub struct Fixpoint {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_attach_blob(
    fixpoint: *mut Fixpoint,
    memory_idx: u32,
    handle: ExternRef,
) {
    unsafe {
        let addr = (1usize << 32) * memory_idx as usize;
        shell::fixpoint_attach_blob(addr as *mut c_void, handle.0);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_create_blob_i64(
    fixpoint: *mut Fixpoint,
    value: u64,
) -> ExternRef {
    ExternRef(unsafe { shell::fixpoint_create_blob_i64(value) })
}
