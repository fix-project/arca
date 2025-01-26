#![no_main]
#![no_std]
#![feature(allocator_api)]

extern crate kernel;

use kernel::{prelude::*, shutdown};

const TRAP_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_trap"));
const IDENTITY_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_identity"));

#[no_mangle]
#[inline(never)]
extern "C" fn kmain() -> ! {
    log::info!("kmain");

    let trap = Lambda::from_elf(TRAP_ELF).apply(Value::None);
    log::info!("running trap program");
    let result = trap.run();
    match result {
        Value::Error(_) => {}
        x => {
            panic!("Expected Error, got {:?}", x);
        }
    };

    let identity = Lambda::from_elf(IDENTITY_ELF).apply(Value::None);
    log::info!("running identity program");
    let result = identity.run();
    assert_eq!(result, Value::None);
    log::info!("done");
    shutdown();
}
