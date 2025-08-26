#![no_std]
#![no_main]

use core::fmt::Write;
use user::buffer::Buffer;
use user::io::{self, File};

extern crate user;

#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let mut ctl = File::options()
        .read(true)
        .write(true)
        .open("/net/tcp/clone")
        .unwrap();

    writeln!(ctl, "announce 0.0.0.0:8080").unwrap();

    let mut id = [0; 32];
    let size = ctl.read(&mut id).unwrap();
    let id = &id[..size];
    let id = core::str::from_utf8(id).unwrap().trim();
    let mut buf: Buffer<32> = Buffer::new();
    write!(&mut buf, "/net/tcp/{id}/listen").unwrap();
    let listen = core::str::from_utf8(&buf).unwrap();

    user::error::log("listening on port 8080");

    loop {
        let mut lctl = File::options().read(true).write(true).open(listen).unwrap();

        if io::fork().unwrap() != 0 {
            continue;
        }

        let mut id = [0; 32];
        let size = lctl.read(&mut id).unwrap();
        let id = &id[..size];
        let id = core::str::from_utf8(id).unwrap().trim();
        let mut buf: Buffer<32> = Buffer::new();
        write!(&mut buf, "/net/tcp/{id}/data").unwrap();
        let buf = core::str::from_utf8(&buf).unwrap();

        let mut ldata = File::options().read(true).write(true).open(buf).unwrap();

        let mut request = [0; 1024];
        let size = ldata.read(&mut request).unwrap();
        let request = &request[..size];

        write!(ldata, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n").unwrap();

        write!(
            ldata,
            r#"
<!doctype html>
<html>
<head>
<title>Hello from Arca!</title>
<meta charset="utf-8"/>
</head>
<body>
<h1>Hello from Arca!</h1>
<p>Your request headers:</p>
<pre>"#
        )
        .unwrap();
        ldata.write(request).unwrap();
        write!(
            ldata,
            r#"</pre>
</body>
</html>
"#
        )
        .unwrap();

        writeln!(lctl, "hangup").unwrap();
        io::exit(0);
    }
}
