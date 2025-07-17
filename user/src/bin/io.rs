#![no_std]
#![no_main]

extern crate user;

use user::prelude::*;

/// Runs an effectful program. Expects two arguments:
///   1. input file
///   2. output file
///
/// and three effect handlers:
///   1. read(key) -> value
///   2. write(key, value)
///   3. exit(value) -> !
#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let read = os::atom("read");
    let write = os::atom("write");
    let exit = os::atom("exit");
    let input = os::prompt();
    let input: Ref<Blob> = input.try_into().unwrap();
    let output = os::prompt();
    let output: Ref<Blob> = output.try_into().unwrap();

    let data = os::call_with_current_continuation(read.apply(input));
    let _ = os::call_with_current_continuation(write.apply(output).apply(data));
    os::exit(exit.apply(os::null()));
}
