#![no_std]
#![no_main]

use core::fmt::Write;

use kernel::host::fs::File;
use kernel::host::net::TcpListener;
use kernel::prelude::*;

#[kmain]
fn main(_: &[usize]) {
    let mut listener = TcpListener::bind(&[0, 0, 0, 0], 8080);
    loop {
        let mut stream = listener.accept();
        let mut buf = [0; 1024];
        let len = stream.recv(&mut buf);
        let slice = core::str::from_utf8(&buf[..len]).unwrap();
        assert!(slice.starts_with("GET "));
        let end = slice[4..].find(" ").unwrap() + 4;
        let mut path = &slice[4..end];

        if path == "/" {
            path = "index.html";
        }
        let file = File::open(path, true, false, false, false, false);
        let Some(mut file) = file else {
            let message = "Not Found\r\n";
            write!(stream, "HTTP/1.1 404 Not Found\r\n").unwrap();
            write!(stream, "Content-Length: {}\r\n", message.len()).unwrap();
            write!(stream, "Content-Type: text/html\r\n").unwrap();
            write!(stream, "\r\n").unwrap();
            write!(stream, "{message}").unwrap();
            stream.close();
            continue;
        };
        let mut buf = [0; 1024];
        let len = file.read(&mut buf);
        file.close();
        let slice = &buf[..len];

        write!(stream, "HTTP/1.1 200 OK\r\n").unwrap();
        write!(stream, "Content-Length: {len}\r\n").unwrap();
        write!(stream, "Content-Type: text/html\r\n").unwrap();
        write!(stream, "\r\n").unwrap();
        stream.send(slice);
        stream.close();
    }
}
