#![no_std]
#![no_main]

extern crate user;

use core::fmt::Write;

use user::prelude::*;

fn accept() {
    Function::symbolic("accept").call_with_current_continuation();
}

#[derive(Debug)]
struct Error;

fn recv(bytes: &mut [u8]) -> Result<usize, Error> {
    let result: Blob = Function::symbolic("recv")
        .apply(bytes.len())
        .call_with_current_continuation()
        .try_into()
        .map_err(|_| Error)?;
    Ok(result.read(0, bytes))
}

fn send(bytes: &[u8]) -> Result<usize, Error> {
    let result: Word = Function::symbolic("send")
        .apply(bytes)
        .call_with_current_continuation()
        .try_into()
        .map_err(|_| Error)?;
    let result = result.read() as i64;
    if result < 0 {
        Err(Error)
    } else {
        Ok(result as usize)
    }
}

fn hangup() {
    Function::symbolic("hangup").call_with_current_continuation();
}

struct Stream;
impl Write for Stream {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        send(s.as_bytes()).map(|_| ()).map_err(|_| core::fmt::Error)
    }
}

/// A simple webserver program; expects a standard filesystem effect interface
/// (open/close/read/write), where stdin is connected to the HTTP request and stdout is connected
/// to the HTTP response.
#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    loop {
        accept();
        let mut bytes: [u8; 1024] = [0; 1024];
        recv(&mut bytes).unwrap();

        let message = "hello, world";
        let len = message.len();

        write!(Stream, "HTTP/1.1 200 OK\r\n").unwrap();
        write!(Stream, "Content-Length: {len}\r\n").unwrap();
        write!(Stream, "Content-Type: text/html\r\n").unwrap();
        write!(Stream, "\r\n").unwrap();

        write!(Stream, "{message}").unwrap();
        hangup();
    }
}
