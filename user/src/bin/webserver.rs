#![no_std]
#![no_main]

extern crate user;

use user::{io::File, prelude::*, print, println};

/// A simple webserver program; expects a standard filesystem effect interface
/// (open/close/read/write), where stdin is connected to the HTTP request and stdout is connected
/// to the HTTP response.
#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let mut stdin = File::STDIN;
    let mut bytes: [u8; 1024] = [0; 1024];
    stdin.read(&mut bytes).unwrap();

    let message = "<h1>Hello, World!</h1>";
    let len = message.len();

    print!("HTTP/1.1 200 OK\r\n");
    print!("Content-Length: {len}\r\n");
    print!("Content-Type: text/html\r\n");
    print!("\r\n");

    println!("{message}");
    io::exit(0)
}
