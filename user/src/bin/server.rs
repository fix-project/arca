#![no_std]
#![no_main]

use core::fmt::Write;
use user::buffer::Buffer;
use user::io::{self, File};

extern crate user;
use user::prelude::*;

struct MVar {
    fd: u64,
}

impl MVar {
    pub fn new() -> MVar {
        let result: Word = Function::symbolic("mvar-new")
            .call_with_current_continuation()
            .try_into()
            .unwrap();
        MVar { fd: result.read() }
    }

    pub fn take(&self) -> Value {
        Function::symbolic("mvar-take")
            .apply(self.fd)
            .call_with_current_continuation()
    }

    pub fn put(&self, value: impl Into<Value>) {
        Function::symbolic("mvar-put")
            .apply(self.fd)
            .apply(value)
            .call_with_current_continuation();
    }
}

impl Drop for MVar {
    fn drop(&mut self) {
        Function::symbolic("close")
            .apply(self.fd)
            .call_with_current_continuation();
    }
}

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

    let mvar = MVar::new();
    mvar.put(0);

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

        let request = core::str::from_utf8(request).unwrap();

        if request.contains("reset") {
            let _ = mvar.take();
            mvar.put(0);
            write!(ldata, "HTTP/1.1 303 See Other\r\nLocation: /\r\n\r\n").unwrap();
        } else {
            write!(ldata, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n").unwrap();
            let number: Word = mvar.take().try_into().unwrap();
            let number = number.read() + 1;
            mvar.put(number);
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
<p>You are request #{number}</p>
<p>Your request headers:</p>
<pre>
{request}
</pre>
</body>
</html>
"#
            )
            .unwrap();
        }

        writeln!(lctl, "hangup").unwrap();
        io::exit(0);
    }
}
