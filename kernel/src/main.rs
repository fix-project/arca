#![no_main]
#![no_std]
#![feature(allocator_api)]
#![feature(new_zeroed_alloc)]

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
    let id: Function = common::elfloader::load_elf(IDENTITY).unwrap();
    let add: Function = common::elfloader::load_elf(ADD).unwrap();
    let curry: Function = common::elfloader::load_elf(CURRY).unwrap();
    let map: Function = common::elfloader::load_elf(MAP).unwrap();
    let io: Function = common::elfloader::load_elf(IO).unwrap();
    let server: Function = common::elfloader::load_elf(SERVER).unwrap();

    // identity function
    log::info!("testing id");
    let x = Value::from(Atom::new("foo"));
    let y = id(x.clone()).force();
    assert_eq!(x, y);

    log::info!("testing add");
    let mut x = Tuple::new(2);
    x.set(0, 10);
    x.set(1, 20);
    let y = add.clone()(x).force();
    assert_eq!(y, 30.into());

    // curry add
    log::info!("testing curry");
    let cadd = curry(add.clone(), 2);
    let y = cadd.clone()(10, 20).force();
    let y: Function = y.try_into().unwrap();
    let y = y.force();
    assert_eq!(y, 30.into());

    // map
    log::info!("testing map");
    let tuple = Tuple::from((1, 2, 3, 4));
    let result = map(cadd(10), tuple).force();
    let result = eval(result);
    let expected = Tuple::from((11, 12, 13, 14)).into();
    assert_eq!(result, expected);

    // I/O effects
    log::info!("testing I/O");
    let f = io("in", "out");
    let mut data = BTreeMap::new();
    data.insert("in".to_string(), Blob::new("hello, world!").into());
    interpret(f, &mut data);
    let value = data.get("out").unwrap();
    assert_eq!(*value, Value::Blob(Blob::new("hello, world!")));

    log::info!("running HTTP server on (vsock) port 80");

    let mut listener = StreamListener::bind(SocketAddr::new(3, 80)).await.unwrap();

    let mut kv = BTreeMap::new();
    kv.insert("count".to_string(), Value::from(0));
    let kv = Arc::new(RwLock::new(kv));

    rt::spawn(async move {
        loop {
            let stream = listener.accept().await.unwrap();
            rt::spawn(interpret_server(server.clone(), kv.clone(), stream));
        }
    });
}

pub fn eval(x: Value) -> Value {
    match x {
        Value::Exception(value) => Exception::new(eval(value.into())).into(),
        Value::Tuple(mut values) => {
            for i in 0..values.len() {
                let x = values.take(i);
                let y = eval(x);
                values.set(i, y);
            }
            Value::Tuple(values)
        }
        Value::Function(f) => eval(f.force()),
        x => x,
    }
}

pub fn interpret(mut thunk: Function, data: &mut BTreeMap<String, Value>) -> Value {
    let read = Blob::new("read");
    let write = Blob::new("write");
    let exit = Blob::new("exit");
    loop {
        let f: Function = thunk.force().try_into().unwrap();
        assert!(f.is_symbolic());
        let inner = f.into_inner();
        let (symbol, mut args) = inner.read();
        let symbol: Blob = symbol.try_into().unwrap();
        assert_eq!(symbol, "effect".into());
        let request: Blob = args.take(0).try_into().unwrap();

        if request == read {
            let file: Blob = args.take(1).try_into().unwrap();
            let s = String::from_utf8(file.into_inner().into_inner().into()).unwrap();
            let value = data.get(&s).unwrap();
            let k: Function = args.take(2).try_into().unwrap();
            thunk = k(value.clone());
        } else if request == write {
            let file: Blob = args.take(1).try_into().unwrap();
            let s = String::from_utf8(file.into_inner().into_inner().into()).unwrap();
            let value = args.take(2);
            data.insert(s, value);
            let k: Function = args.take(3).try_into().unwrap();
            thunk = k(Null::new());
        } else if request == exit {
            return args.take(1);
        } else {
            panic!("invalid effect: {request:?}")
        }
    }
}

pub async fn interpret_server(
    mut f: Function,
    kv: Arc<RwLock<BTreeMap<String, Value>>>,
    mut stream: Stream,
) {
    let get = Blob::new("get");
    let set = Blob::new("set");
    let cas = Blob::new("compare-and-swap");
    let recv = Blob::new("recv");
    let send = Blob::new("send");
    let close = Blob::new("close");

    loop {
        let y: Function = f.force().try_into().unwrap();

        assert!(y.is_symbolic());
        let inner = y.into_inner();
        let (symbol, mut args) = inner.read();
        let symbol: Blob = symbol.try_into().unwrap();
        assert_eq!(symbol, "effect".into());

        let request: Blob = args.take(0).try_into().unwrap();

        if request == get {
            let file: Blob = args.take(1).try_into().unwrap();
            let key = String::from_utf8(file.into_inner().into_inner().into()).unwrap();
            let kv = kv.read();
            let value = kv.get(&key).cloned().unwrap_or_default();
            let k: Function = args.take(2).try_into().unwrap();
            f = k(value);
        } else if request == set {
            let file: Blob = args.take(1).try_into().unwrap();
            let key = String::from_utf8(file.into_inner().into_inner().into()).unwrap();
            let value = args.take(2);
            let mut kv = kv.write();
            let old = kv.insert(key, value).unwrap_or_default();
            let k: Function = args.take(3).try_into().unwrap();
            f = k(old);
        } else if request == recv {
            let x = stream.read().await.unwrap();
            let k: Function = args.take(1).try_into().unwrap();
            f = k(&*x);
        } else if request == send {
            let data: Blob = args.take(1).try_into().unwrap();
            stream.write(&data).await.unwrap();
            let k: Function = args.take(2).try_into().unwrap();
            f = k(None);
        } else if request == cas {
            let key: Blob = args.take(1).try_into().unwrap();
            let key = String::from_utf8(key.into_inner().into_inner().into()).unwrap();
            let expected = args.take(2);
            let replacement = args.take(3);
            let k: Function = args.take(4).try_into().unwrap();
            let mut kv = kv.write();
            let default = Value::default();
            let old = kv.get(&key).unwrap_or(&default);
            if old == &expected {
                let old = kv.insert(key, replacement).unwrap_or_default();
                f = k(old);
            } else {
                f = k(Value::Exception(Exception::new(replacement)));
            }
        } else if request == close {
            stream.close().await.unwrap();
            return;
        } else {
            panic!("invalid effect: {request:?}");
        }
    }
}
