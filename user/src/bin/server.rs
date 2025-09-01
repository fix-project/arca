#![no_std]
#![no_main]
#![feature(maybe_uninit_array_assume_init)]
#![feature(try_blocks)]
#![allow(dead_code)]

extern crate alloc;

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::{format, vec};
use bincode::error::{DecodeError, EncodeError};
use chumsky::error::Rich;
use chumsky::{Parser, extra};
use core::fmt::Write;
use core::hash::{Hash as _, Hasher, SipHasher};
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::str::Utf8Error;
use user::buffer::Buffer;
use user::io::{self, File};

extern crate user;
use user::prelude::*;

struct Monitor {
    fd: u64,
}

impl Monitor {
    pub fn new() -> Monitor {
        let result: Word = Function::symbolic("monitor-new")
            .call_with_current_continuation()
            .try_into()
            .unwrap();
        Monitor { fd: result.read() }
    }

    pub fn enter(&self, f: impl FnOnce(&mut MonitorContext) -> Value) -> Value {
        if let Ok(k) = user::os::continuation() {
            Function::symbolic("monitor-enter")
                .apply(self.fd)
                .apply(k)
                .call_with_current_continuation()
        } else {
            let mut ctx = MonitorContext;
            let value = f(&mut ctx);
            Function::symbolic("exit")
                .apply(value)
                .call_with_current_continuation()
        }
    }

    pub fn set(&self, value: impl Into<Value>) -> Value {
        self.enter(|ctx| ctx.set(value.into()))
    }

    pub fn get(&self) -> Value {
        self.enter(|ctx| ctx.get())
    }
}

struct MonitorContext;

impl MonitorContext {
    pub fn get(&self) -> Value {
        Function::symbolic("get").call_with_current_continuation()
    }

    pub fn set(&mut self, value: impl Into<Value>) -> Value {
        Function::symbolic("set")
            .apply(value)
            .call_with_current_continuation()
    }

    pub fn wait(&self) {
        Function::symbolic("wait").call_with_current_continuation();
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        Function::symbolic("close")
            .apply(self.fd)
            .call_with_current_continuation();
    }
}

#[derive(Debug)]
enum Command {
    Set,
    Add,
    Replace,
    Append,
    Prepend,
}

#[derive(Debug)]
struct Storage {
    command: Command,
    key: String,
    flags: u16,
    exptime: u32,
    value: usize,
}

fn command<'a>() -> impl Parser<'a, &'a [u8], Command, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    choice((just(b"set").map(|_| Command::Set),)).labelled("command")
}

fn ident<'a>() -> impl Parser<'a, &'a [u8], String, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    any()
        .filter(|x: &u8| x.is_ascii_alphanumeric() || *x == b'-')
        .repeated()
        .at_least(1)
        .collect()
        .map(|x: Vec<u8>| String::from_utf8(x))
        .unwrapped()
        .labelled("ident")
}

fn parse_u16<'a>() -> impl Parser<'a, &'a [u8], u16, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    any()
        .filter(|x: &u8| x.is_ascii_digit())
        .repeated()
        .at_least(1)
        .collect()
        .map(|x: Vec<u8>| String::from_utf8(x))
        .unwrapped()
        .map(|x: String| x.parse())
        .unwrapped()
        .labelled("u16")
}

fn parse_u32<'a>() -> impl Parser<'a, &'a [u8], u32, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    any()
        .filter(|x: &u8| x.is_ascii_digit())
        .repeated()
        .at_least(1)
        .collect()
        .map(|x: Vec<u8>| String::from_utf8(x))
        .unwrapped()
        .map(|x: String| x.parse())
        .unwrapped()
        .labelled("u32")
}

fn parse_usize<'a>() -> impl Parser<'a, &'a [u8], usize, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    any()
        .filter(|x: &u8| x.is_ascii_digit())
        .repeated()
        .at_least(1)
        .collect()
        .map(|x: Vec<u8>| String::from_utf8(x))
        .unwrapped()
        .map(|x: String| x.parse())
        .unwrapped()
        .labelled("usize")
}

fn storage<'a>() -> impl Parser<'a, &'a [u8], Storage, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    command()
        .then_ignore(just(b' '))
        .then(ident().then_ignore(just(b' ')))
        .then(parse_u16().then_ignore(just(b' ')))
        .then(parse_u32().then_ignore(just(b' ')))
        .then(parse_usize().then_ignore(just(b"\r\n")))
        .map(|((((command, key), flags), exptime), value)| Storage {
            command,
            key,
            flags,
            exptime,
            value,
        })
        .labelled("storage request")
}

#[derive(Debug)]
enum Request {
    Storage(Storage),
    Get(Vec<String>),
    Delete(String),
}

fn get<'a>() -> impl Parser<'a, &'a [u8], Vec<String>, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    just(b"get ")
        .ignore_then(
            ident()
                .then_ignore(just(b' ').or_not())
                .repeated()
                .collect(),
        )
        .then_ignore(just(b"\r\n"))
        .labelled("get request")
}

fn delete<'a>() -> impl Parser<'a, &'a [u8], String, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    just(b"delete ")
        .ignore_then(ident())
        .then_ignore(just(b"\r\n"))
        .labelled("delete request")
}

fn request<'a>() -> impl Parser<'a, &'a [u8], Request, extra::Err<Rich<'a, u8>>> {
    use chumsky::prelude::*;
    choice((
        storage().map(Request::Storage),
        get().map(Request::Get),
        delete().map(Request::Delete),
    ))
    .labelled("request")
}

struct Error(String);

impl From<EncodeError> for Error {
    fn from(e: EncodeError) -> Self {
        Error(e.to_string())
    }
}

impl From<DecodeError> for Error {
    fn from(e: DecodeError) -> Self {
        Error(e.to_string())
    }
}

impl From<user::io::Error> for Error {
    fn from(_: user::io::Error) -> Self {
        Error("I/O".to_string())
    }
}

impl From<core::fmt::Error> for Error {
    fn from(e: core::fmt::Error) -> Self {
        Error(e.to_string())
    }
}

impl From<Value> for Error {
    fn from(v: Value) -> Self {
        Error(format!("conversion of {v:?}"))
    }
}

impl<T: core::fmt::Debug> From<Vec<T>> for Error {
    fn from(v: Vec<T>) -> Self {
        Error(format!("conversion of {v:?}"))
    }
}

impl From<Utf8Error> for Error {
    fn from(e: Utf8Error) -> Self {
        Error(e.to_string())
    }
}

pub fn cons(car: impl Into<Value>, cdr: impl Into<Value>) -> Tuple {
    let mut cons = Tuple::new(2);
    cons.set(0, car);
    cons.set(1, cdr);
    cons
}

pub fn nil() -> Tuple {
    Tuple::new(0)
}

pub fn is_nil(x: &Tuple) -> bool {
    x.is_empty()
}

pub fn car(cons: &Tuple) -> Value {
    cons.get(0)
}

pub fn cdr(cons: &Tuple) -> Value {
    cons.get(1)
}

pub fn alist_search(alist: &Tuple, key: &str) -> Option<(Box<[u8]>, u16)> {
    if is_nil(alist) {
        return None;
    }
    let car = car(alist);

    let entry: Tuple = car.try_into().unwrap();
    let k: Blob = entry.get(0).try_into().unwrap();
    if k.with_ref(|k| k == key.as_bytes()) {
        let v: Blob = entry.get(1).try_into().unwrap();
        let m: Word = entry.get(2).try_into().unwrap();
        return Some((v.with_ref(|v| v.to_owned()).into(), m.read() as u16));
    }

    let cdr = cdr(alist);
    alist_search(&cdr.try_into().unwrap(), key)
}

pub fn alist_insert(alist: Tuple, key: String, value: Box<[u8]>, meta: u16) -> Tuple {
    let mut entry = Tuple::new(3);
    entry.set(0, &*key);
    entry.set(1, &*value);
    entry.set(2, meta as u64);
    cons(entry, alist)
}

pub fn alist_remove(alist: Tuple, key: &str) -> Tuple {
    if is_nil(&alist) {
        return alist;
    }
    let cdr = cdr(&alist);
    let cdr_ = alist_remove(cdr.try_into().unwrap(), key);

    let entry: Tuple = car(&alist).try_into().unwrap();
    let k: Blob = entry.get(0).try_into().unwrap();
    if k.with_ref(|k| k == key.as_bytes()) {
        return cdr_;
    }

    let car = car(&alist);
    cons(car, cdr_)
}

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

    let map = Monitor::new();
    map.set(nil());

    loop {
        let mut lctl = File::options().read(true).write(true).open(listen).unwrap();

        if io::fork().unwrap().is_some() {
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

        let result: Result<(), Error> = try {
            loop {
                let mut bytes = vec![];
                loop {
                    let mut byte = [0];
                    if ldata.read(&mut byte)? == 0 {
                        break;
                    }
                    bytes.push(byte[0]);
                    if byte[0] == b'\n' {
                        break;
                    }
                }
                let request = request().parse(&bytes).into_result()?;

                match request {
                    Request::Storage(request) => {
                        let mut value = vec![0; request.value + 2];
                        ldata.read(&mut value)?;
                        value.pop();
                        value.pop();
                        let response = match request.command {
                            Command::Set => {
                                map.enter(|ctx| {
                                    let alist = ctx.get().try_into().unwrap();
                                    let alist = alist_insert(
                                        alist,
                                        request.key,
                                        value.into(),
                                        request.flags,
                                    );
                                    ctx.set(alist);
                                    Value::default()
                                });
                                "STORED\r\n"
                            }
                            _ => todo!(),
                        };
                        ldata.write_str(response)?;
                    }
                    Request::Get(items) => {
                        for item in items {
                            let result = map.enter(|ctx| {
                                let alist = ctx.get().try_into().unwrap();
                                if let Some((value, flags)) = alist_search(&alist, &item) {
                                    let mut tuple = Tuple::new(2);
                                    tuple.set(0, &value[..]);
                                    tuple.set(1, flags as u64);
                                    tuple.into()
                                } else {
                                    Value::default()
                                }
                            });
                            if let Ok(x) = Tuple::try_from(result) {
                                let flags = Word::try_from(x.get(1))?.read() as u16;
                                let blob = Blob::try_from(x.get(0))?;
                                let bytes = blob.len();
                                let mut value = vec![0; bytes];
                                blob.read(0, &mut value);
                                write!(ldata, "VALUE {item} {flags} {bytes}\r\n")?;
                                ldata.write(&value)?;
                                ldata.write(b"\r\n")?;
                            }
                        }
                        ldata.write(b"END\r\n")?;
                    }
                    Request::Delete(key) => {
                        let result: Word = map
                            .enter(|ctx| {
                                let alist = ctx.get().try_into().unwrap();
                                let alist = alist_remove(alist, &key);
                                ctx.set(alist);
                                1.into()
                            })
                            .try_into()?;
                        if result.read() == 0 {
                            ldata.write(b"NOT_FOUND\r\n")?;
                        } else {
                            ldata.write(b"DELETED\r\n")?;
                        }
                    }
                }
            }
        };
        let result = result.unwrap_err();
        let _ = writeln!(ldata, "SERVER_ERROR {}", result.0);
        let _ = writeln!(lctl, "hangup");
        crate::io::exit(0);
    }
}
