#![no_std]
#![no_main]

use kernel::host::os;
use kernel::prelude::*;

#[kmain]
fn main() {
    let args = os::argv();
    let name = if args.len() >= 2 { &args[1] } else { "world" };
    log::info!("hello, {name}");
}
