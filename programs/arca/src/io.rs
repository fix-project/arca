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
    let eff = Function::symbolic("effect");
    let effect = |s| eff.clone()(s);
    let input = os::argument();
    let input: Blob = input.try_into().unwrap();
    let output = os::argument();
    let output: Blob = output.try_into().unwrap();

    let data = effect("read")(input)(Continuation);
    let _ = effect("write")(output, data)(Continuation);
    effect("exit")(Continuation);

    unreachable!("exit effect should have exited!");
}
