#![no_main]
#![no_std]

use kernel::host::os;
use kernel::prelude::*;

#[kmain]
fn main() {
    let argv = os::argv();
    let arguments = fixruntime::cli::configure_directory(&argv[1..]).unwrap_or_else(|error| {
        println!("fix: {error}");
        kernel::exit(2);
    });

    let result = if arguments == ["--session"] {
        fixruntime::cli::serve().map(|()| 0)
    } else {
        fixruntime::cli::run(arguments)
    };
    match result {
        Ok(0) => (),
        Ok(status) => kernel::exit(status),
        Err(error) => {
            println!("fix: {error}");
            kernel::exit(2);
        }
    }

    kernel::shutdown();
}
