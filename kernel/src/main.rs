#![no_main]
#![no_std]
#![feature(allocator_api)]
#![feature(new_zeroed_alloc)]

use core::time::Duration;

use alloc::collections::btree_map::BTreeMap;
use alloc::string::String;
use alloc::string::ToString as _;
use alloc::sync::Arc;
use common::util::rwlock::RwLock;
use kernel::prelude::*;
use kernel::rt;
use kernel::virtio::vsock::SocketAddr;
use kernel::virtio::vsock::Stream;
use kernel::virtio::vsock::StreamListener;
use macros::kmain;

extern crate alloc;
extern crate kernel;

const IDENTITY: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_identity"));
const ADD: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_add"));
const CURRY: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_curry"));
const MAP: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_map"));
const IO: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_io"));
const SERVER: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_server"));

#[kmain]
async fn kmain(args: &[usize]) {
    log::info!("setting up (have {} args)", args.len());
    let id: Lambda = Thunk::from_elf(IDENTITY).run().try_into().unwrap();
    let add: Lambda = Thunk::from_elf(ADD).run().try_into().unwrap();
    let curry: Lambda = Thunk::from_elf(CURRY).run().try_into().unwrap();
    let map: Lambda = Thunk::from_elf(MAP).run().try_into().unwrap();
    let io: Lambda = Thunk::from_elf(IO).run().try_into().unwrap();
    let server: Thunk = Thunk::from_elf(SERVER);

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

    let mut listener = StreamListener::bind(SocketAddr::new(3, 80)).await.unwrap();

    let mut kv = BTreeMap::new();
    kv.insert("count".to_string(), Value::Word(0.into()));
    let kv = Arc::new(RwLock::new(kv));

    rt::spawn(async move {
        loop {
            let stream = listener.accept().await.unwrap();
            rt::spawn(interpret_server(server.clone(), kv.clone(), stream));
        }
    });

    rt::spawn(async move {
        loop {
            log::info!(
                "used: {} bytes ({:.2} %)",
                BuddyAllocator.used_size(),
                BuddyAllocator.usage() * 100.
            );
            rt::delay_for(Duration::from_millis(500)).await;
        }
    });
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

pub async fn interpret_server(
    mut thunk: Thunk,
    kv: Arc<RwLock<BTreeMap<String, Value>>>,
    mut stream: Stream,
) {
    loop {
        let result: Thunk = thunk.run().try_into().unwrap();
        let get = Atom::new("get");
        let set = Atom::new("set");
        let cas = Atom::new("compare-and-swap");
        let recv = Atom::new("recv");
        let send = Atom::new("send");
        let close = Atom::new("close");
        let (symbol, args) = result.symbolic().unwrap();
        let symbol: Atom = symbol.clone().try_into().unwrap();
        if symbol == get {
            let file: Blob = args[0].clone().try_into().unwrap();
            let key = String::from_utf8(file.into_inner().into()).unwrap();
            let kv = kv.read();
            let value = kv.get(&key).cloned().unwrap_or(Value::Null);
            let k: Lambda = args[1].clone().try_into().unwrap();
            thunk = k.apply(value);
        } else if symbol == set {
            let file: Blob = args[0].clone().try_into().unwrap();
            let key = String::from_utf8(file.into_inner().into()).unwrap();
            let value = args[1].clone();
            let mut kv = kv.write();
            let old = kv.insert(key, value).unwrap_or(Value::Null);
            let k: Lambda = args[2].clone().try_into().unwrap();
            thunk = k.apply(old);
        } else if symbol == recv {
            let x = stream.read().await.unwrap();
            let k: Lambda = args[0].clone().try_into().unwrap();
            thunk = k.apply(Value::Blob(Blob::new(x)));
        } else if symbol == send {
            let data: Blob = args[0].clone().try_into().unwrap();
            stream.write(&data).await.unwrap();
            let k: Lambda = args[1].clone().try_into().unwrap();
            thunk = k.apply(Value::Null);
        } else if symbol == cas {
            let key: Blob = args[0].clone().try_into().unwrap();
            let key = String::from_utf8(key.into_inner().into()).unwrap();
            let expected = args[1].clone();
            let replacement = args[2].clone();
            let k: Lambda = args[3].clone().try_into().unwrap();
            let mut kv = kv.write();
            let old = kv.get(&key).unwrap_or(&Value::Null);
            if old == &expected {
                let old = kv.insert(key, replacement).unwrap_or(Value::Null);
                thunk = k.apply(old);
            } else {
                thunk = k.apply(Value::Error(Error::new(replacement)));
            }
        } else if symbol == close {
            stream.close().await.unwrap();
            return;
        } else {
            panic!("invalid effect: {symbol:?}");
        }
    }
}
