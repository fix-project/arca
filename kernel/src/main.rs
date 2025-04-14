#![no_main]
#![no_std]
#![feature(allocator_api)]

use core::fmt::Write;
use core::time::Duration;

use kernel::debugcon::CONSOLE;
use kernel::prelude::*;
use kernel::rt;
use macros::kmain;

extern crate alloc;
extern crate kernel;

const FORCER: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_forcer"));
const FORCEE: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_forcee"));

#[kmain]
async fn kmain(_: &[usize]) {
    log::info!("no clone");
    let kib = 1024;
    let mib = 1024 * kib;
    // let gib = 1024 * mib;
    let values = [
        0,
        4 * kib,
        8 * kib,
        16 * kib,
        32 * kib,
        64 * kib,
        128 * kib,
        256 * kib,
        512 * kib,
        mib,
        2 * mib,
        4 * mib,
        8 * mib,
        16 * mib,
        32 * mib,
        64 * mib,
        128 * mib,
        256 * mib,
        512 * mib,
    ];
    let duration = Duration::from_secs(10);
    let mut console = CONSOLE.lock();
    writeln!(*console, "(optimized) bytes, iterations, duration",).unwrap();
    for bytes in values {
        let count = bytes / 4096;
        let count = test(count, true);
        if count != 0 {
            log::info!(
                "{count} ({bytes} bytes, {:?} per iteration)",
                duration / count as u32
            );
        }
        writeln!(*console, "{bytes},{count},{}", duration.as_secs_f64()).unwrap();
    }
    log::info!("with clone");
    writeln!(*console, "(naive) bytes, iterations, duration",).unwrap();
    for bytes in values {
        let count = bytes / 4096;
        let count = test(count, false);
        if count != 0 {
            log::info!(
                "{count} ({bytes} bytes, {:?} per iteration)",
                duration / count as u32
            );
        }
        writeln!(*console, "{bytes},{count},{}", duration.as_secs_f64()).unwrap();
    }
}

fn test(count: usize, unified: bool) -> usize {
    let forcer = Thunk::from_elf(FORCER);
    let Value::Lambda(forcer) = forcer.run() else {
        panic!();
    };
    let forcee = Thunk::from_elf(FORCEE);
    let Value::Lambda(forcee) = forcee.run() else {
        panic!();
    };
    let forcee = forcee.apply(Value::Word(count as u64));
    let Value::Lambda(forcee) = forcee.run() else {
        panic!();
    };
    let forcee = forcee.apply(Value::Word(unified as u64));
    let forcer = forcer.apply(Value::Thunk(forcee));

    let Value::Word(count) = forcer.run() else {
        panic!();
    };
    count as usize
}
