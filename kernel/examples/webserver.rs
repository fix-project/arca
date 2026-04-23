#![no_std]
#![no_main]

use common::elfloader;
use kernel::host::net::TcpListener;
use kernel::{kthread, prelude::*};

const HANDLER: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_webserver"));

#[kmain]
fn main(_: &[usize]) {
    let mut listener = TcpListener::bind(&[0, 0, 0, 0], 8080);
    let handler: Function = elfloader::load_elf(HANDLER).unwrap();
    loop {
        let mut stream = listener.accept();
        let handler = handler.clone();
        kthread::spawn(move || {
            let mut handler = handler;
            loop {
                let effect: Function = handler
                    .force()
                    .try_into()
                    .expect("handler returned non-effect");
                let mut data: Tuple = effect.read().try_into().expect("could not read effect");
                let t: Blob = data.take(0).try_into().unwrap();
                assert_eq!(&*t, b"Symbolic");
                let effect: Blob = data.take(1).try_into().expect("could not find effect name");
                let args: Tuple = data
                    .take(2)
                    .try_into()
                    .expect("could not find effect arguments");
                let mut args: Vec<Value> = args.into_iter().collect();
                let k: Function = args
                    .pop()
                    .and_then(|x| x.try_into().ok())
                    .expect("could not find continuation");
                handler = match (&*effect, &*args) {
                    (b"read", &[Value::Word(fd), Value::Word(len)]) => {
                        assert_eq!(fd.read(), 0);
                        let mut v = vec![0; len.read() as usize];
                        let len = stream.recv(&mut v);
                        v.truncate(len);
                        k.apply(Blob::new(v))
                    }
                    (b"write", &[Value::Word(fd), Value::Blob(ref data)]) => {
                        assert_eq!(fd.read(), 1);
                        let len = stream.send(&*data);
                        k.apply(Word::new(len as u64))
                    }
                    (b"close", &[Value::Word(_)]) => k.apply(None),
                    (b"exit", &[Value::Word(_)]) => {
                        break;
                    }
                    (e, _) => {
                        let e: &str = str::from_utf8(e).unwrap();
                        panic!("unhandled effect: {e}");
                    }
                };
            }
        });
    }
}
