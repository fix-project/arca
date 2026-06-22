#![no_std]
#![no_main]

use common::elfloader;
use kernel::host::net::TcpListener;
use kernel::{kthread, prelude::*};

const HANDLER: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_webserver2"));

#[kmain]
fn main() {
    kthread::wfi();
    let listener = Arc::new(TcpListener::bind(&[0, 0, 0, 0], 8081));
    log::info!("listening on port 8081");

    for _ in 0..kernel::ncores() {
        let listener = listener.clone();
        kthread::spawn(move || {
            let mut handler: Function = elfloader::load_elf(HANDLER).unwrap();
            let mut current = None;
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
                    (b"accept", &[]) => {
                        current = Some(listener.accept());
                        k.apply(None)
                    }
                    (b"recv", &[Value::Word(len)]) => {
                        if let Some(ref mut stream) = current {
                            let mut v = vec![0; len.read() as usize];
                            let len = stream.recv(&mut v);
                            v.truncate(len);
                            k.apply(Blob::new(v))
                        } else {
                            k.apply(None)
                        }
                    }
                    (b"send", &[Value::Blob(ref data)]) => {
                        if let Some(ref mut stream) = current {
                            let len = stream.send(&*data);
                            k.apply(Word::new(len as u64))
                        } else {
                            k.apply(None)
                        }
                    }
                    (b"hangup", &[]) => {
                        current.take();
                        k.apply(None)
                    }
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
