#![no_std]
#![no_main]

use kernel::host::net::TcpListener;
use kernel::prelude::*;
use kernel::rt;

#[kmain]
async fn main(_: &[usize]) {
    let listener = TcpListener::bind(&[0, 0, 0, 0], 8080);
    loop {
        let stream = listener.accept();
        let mut buf = [0; 1024];
        stream.recv(&mut buf);
        stream.send(b"HTTP/1.1 200 OK\r\n");
        stream.send(b"Content-Length: 7\r\n");
        stream.send(b"Content-Type: text/html\r\n");
        stream.send(b"\r\n");
        stream.send(b"hello\r\n");
        stream.close();
    }
}
