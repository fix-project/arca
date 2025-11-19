#![no_std]
#![no_main]
#![feature(maybe_uninit_array_assume_init)]
#![feature(auto_traits)]
#![feature(negative_impls)]
#![feature(try_blocks)]
#![allow(dead_code)]

extern crate alloc;

mod error;
mod kvstore;
mod proto;
mod shared;

use alloc::vec;
use chumsky::Parser;
use core::fmt::Write;
use user::buffer::Buffer;
use user::io::{self, Buffered, File};

extern crate user;

use error::*;
use kvstore::*;
use proto::*;

#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let mut ctl = File::options()
        .read(true)
        .write(true)
        .open("/net/tcp/clone")
        .unwrap();

    writeln!(ctl, "announce 0.0.0.0:11211").unwrap();

    let mut id = [0; 32];
    let size = ctl.read(&mut id).unwrap();
    let id = &id[..size];
    let id = core::str::from_utf8(id).unwrap().trim();
    let mut buf: Buffer<32> = Buffer::new();
    write!(&mut buf, "/net/tcp/{id}/listen").unwrap();
    let listen = core::str::from_utf8(&buf).unwrap();

    user::error::log("listening on port 11211");

    let kv = KVStore::new(256);

    loop {
        let mut lctl = File::options().read(true).write(true).open(listen).unwrap();

        if io::fork().unwrap().is_some() {
            core::mem::drop(lctl);
            continue;
        }
        core::mem::drop(ctl);

        let mut id = [0; 32];
        let size = lctl.read(&mut id).unwrap();
        let id = &id[..size];
        let id = core::str::from_utf8(id).unwrap().trim();
        let mut buf: Buffer<32> = Buffer::new();
        write!(&mut buf, "/net/tcp/{id}/data").unwrap();
        let buf = core::str::from_utf8(&buf).unwrap();

        let ldata = File::options().read(true).write(true).open(buf).unwrap();
        let mut ldata = Buffered::new(ldata);

        let result: Result<(), Error> = try {
            loop {
                let bytes = ldata.read_until(b'\n').map_err(|e| e.into())?;
                let request = request()
                    .parse(&bytes)
                    .into_result()
                    .map_err(|e| e.into())?;

                match request {
                    Request::Storage(request) => {
                        let mut value = vec![0; request.value + 2];
                        ldata.read(&mut value).map_err(|e| e.into())?;
                        value.pop();
                        value.pop();
                        let response = match request.command {
                            Command::Set => {
                                kv.insert(request.key.as_bytes(), &value, request.flags);
                                "STORED\r\n"
                            }
                            _ => todo!(),
                        };
                        ldata.write_str(response).map_err(|e| e.into())?;
                    }
                    Request::Get(items) => {
                        for item in items {
                            let result = kv.lookup(item.as_bytes());
                            if let Some((value, flags)) = result {
                                let bytes = value.len();
                                write!(ldata, "VALUE {item} {flags} {bytes}\r\n")
                                    .map_err(|e| e.into())?;
                                value
                                    .with_ref(|value| ldata.write(value))
                                    .map_err(|e| e.into())?;
                                ldata.write(b"\r\n").map_err(|e| e.into())?;
                            }
                        }
                        ldata.write(b"END\r\n").map_err(|e| e.into())?;
                    }
                    Request::Delete(_) => {
                        let _ = ldata.write(b"CLIENT_ERROR\r\n");
                    }
                }
            }
        };
        let result = result.unwrap_err();
        let _ = writeln!(ldata, "SERVER_ERROR {result}");
        let _ = writeln!(lctl, "hangup");
        crate::io::exit(0);
    }
}
