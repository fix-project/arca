#![no_main]
#![no_std]
#![feature(allocator_api)]
#![feature(new_zeroed_alloc)]

use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::string::String;
use alloc::string::ToString as _;
use kernel::prelude::*;
use kernel::rt;
use kernel::virtio::VSock;
use macros::kmain;

extern crate alloc;
extern crate kernel;

const IDENTITY: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_identity"));
const ADD: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_add"));
const CURRY: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_curry"));
const MAP: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_map"));
const IO: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_io"));

#[kmain]
async fn kmain(args: &[usize]) {
    log::info!("setting up (have {} args)", args.len());
    let id: Lambda = Thunk::from_elf(IDENTITY).run().try_into().unwrap();
    let add: Lambda = Thunk::from_elf(ADD).run().try_into().unwrap();
    let curry: Lambda = Thunk::from_elf(CURRY).run().try_into().unwrap();
    let map: Lambda = Thunk::from_elf(MAP).run().try_into().unwrap();
    let io: Lambda = Thunk::from_elf(IO).run().try_into().unwrap();

    // identity function
    log::info!("testing id");
    let x = Value::Atom(Atom::new("foo"));
    let y = id.apply(x.clone()).run();
    assert_eq!(x, y);

    // add
    log::info!("testing add");
    let x = Tree::new([10.into(), 20.into()]);
    let y = add.clone().apply(x).run();
    assert_eq!(y, 30.into());

    // curry add
    log::info!("testing curry");
    let x = add.clone();
    let n = 2;
    let Value::Lambda(cadd) = curry.apply(x).run() else {
        panic!();
    };
    let Value::Lambda(cadd) = cadd.apply(n).run() else {
        panic!();
    };
    let Value::Lambda(cadd) = cadd.apply(10).run() else {
        panic!();
    };
    let y = cadd.clone().apply(20).run();
    let y: Thunk = y.try_into().unwrap();
    let y = y.run();
    assert_eq!(y, 30.into());

    // map
    log::info!("testing map");
    let tuple = Tree::new([1.into(), 2.into(), 3.into(), 4.into()]);
    let Value::Lambda(map) = map.apply(Value::Lambda(cadd)).run() else {
        panic!();
    };
    let result = map.apply(tuple).run();
    let result = eval(result);
    let expected = Tree::new([11.into(), 12.into(), 13.into(), 14.into()]).into();
    assert_eq!(result, expected);

    // I/O effects
    log::info!("testing I/O");
    let f: Lambda = io.apply(Blob::from("in")).run().try_into().unwrap();
    let f: Thunk = f.apply(Blob::from("out"));
    let mut data = BTreeMap::new();
    data.insert("in".to_string(), Blob::from("hello, world!").into());
    let result = interpret(f, &mut data);
    assert_eq!(result, Value::Null);
    let value = data.get("out").unwrap();
    assert_eq!(*value, Value::Blob(Blob::from("hello, world!")));

    'main: loop {
        let packet = VSock.read().await;
        let header: *mut Header = packet.as_ptr() as *mut Header;
        let header = unsafe { header.read() };
        log::info!("header: {header:?}");
        if header.op != 1 {
            continue 'main;
        }
        // assert_eq!(header.ptype, 1); // stream
        // assert_eq!(header.op, 1); // request
        // assert_eq!(header.dst_port, 80); // HTTP?

        let packet = Box::new(Header {
            src_cid: 3,
            dst_cid: 2,
            src_port: 80,
            dst_port: header.src_port,
            len: 0,
            ptype: 1,
            op: 2,
            flags: 0,
            buf_alloc: 1024,
            fwd_cnt: 0,
        });
        let packet: Box<[u8; core::mem::size_of::<Header>()]> =
            unsafe { core::mem::transmute(packet) };
        VSock.write(packet).await;
        log::info!("sent reply");
        loop {
            let packet = VSock.read().await;
            let header: *mut Header = packet.as_ptr() as *mut Header;
            let header = unsafe { header.read() };
            log::info!("header: {header:?}");
            if header.op == 5 {
                let port = header.src_port;
                let payload = &packet[core::mem::size_of::<Header>()..];
                log::info!("payload: {payload:?}");

                let payload = b"hello, world!\n";
                let header = Header {
                    src_cid: 3,
                    dst_cid: 2,
                    src_port: 80,
                    dst_port: port,
                    len: payload.len() as u32,
                    ptype: 1,
                    op: 5,
                    flags: 0,
                    buf_alloc: 1024,
                    fwd_cnt: 0,
                };
                let mut buf = unsafe {
                    Box::new_zeroed_slice(core::mem::size_of::<Header>() + payload.len())
                        .assume_init()
                };
                let header: Box<[u8; core::mem::size_of::<Header>()]> =
                    unsafe { core::mem::transmute(Box::new(header)) };
                buf[0..header.len()].copy_from_slice(&header[..]);
                buf[header.len()..].copy_from_slice(payload);
                VSock.write(buf).await;

                let packet = Box::new(Header {
                    src_cid: 3,
                    dst_cid: 2,
                    src_port: 80,
                    dst_port: port,
                    len: 0,
                    ptype: 1,
                    op: 4,
                    flags: 3,
                    buf_alloc: 1024,
                    fwd_cnt: 0,
                });
                let packet: Box<[u8; core::mem::size_of::<Header>()]> =
                    unsafe { core::mem::transmute(packet) };
                VSock.write(packet).await;
                continue 'main;
            }
        }
    }
}

pub fn eval(x: Value) -> Value {
    match x {
        Value::Error(value) => Error::new(eval(value.into())).into(),
        Value::Tree(values) => Value::Tree(values.iter().map(|x| eval(x.clone())).collect()),
        Value::Thunk(thunk) => eval(thunk.run()),
        x => x,
    }
}

pub fn interpret(mut thunk: Thunk, data: &mut BTreeMap<String, Value>) -> Value {
    loop {
        let result: Thunk = thunk.run().try_into().unwrap();
        let read = Atom::new("read");
        let write = Atom::new("write");
        let exit = Atom::new("exit");
        let (symbol, args) = result.symbolic().unwrap();
        let symbol: Atom = symbol.clone().try_into().unwrap();
        if symbol == read {
            let file: Blob = args[0].clone().try_into().unwrap();
            let s = String::from_utf8(file.into_inner().into()).unwrap();
            let value = data.get(&s).unwrap();
            let k = args[1].clone();
            thunk = k.apply(value.clone());
        } else if symbol == write {
            let file: Blob = args[0].clone().try_into().unwrap();
            let s = String::from_utf8(file.into_inner().into()).unwrap();
            let value = args[1].clone();
            data.insert(s, value);
            let k = args[2].clone();
            thunk = k.apply(Value::Null);
        } else if symbol == exit {
            return args[0].clone();
        } else {
            panic!("invalid effect")
        }
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
struct Header {
    src_cid: u64,
    dst_cid: u64,
    src_port: u32,
    dst_port: u32,
    len: u32,
    ptype: u16,
    op: u16,
    flags: u32,
    buf_alloc: u32,
    fwd_cnt: u32,
}

const _: () = const {
    assert!(core::mem::size_of::<Header>() == 44);
};
