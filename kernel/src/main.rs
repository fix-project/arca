#![no_main]
#![no_std]
#![feature(allocator_api)]

extern crate alloc;
extern crate kernel;

use alloc::vec;

use kernel::{prelude::*, shutdown};

const TRAP_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_trap"));
const IDENTITY_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_identity"));
const ADD_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_add"));
const ERROR_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_error"));

#[no_mangle]
#[inline(never)]
extern "C" fn kmain() -> ! {
    log::info!("kmain");

    let trap = Lambda::from_elf(TRAP_ELF).apply(Value::Null);
    log::info!("running trap program");
    let result = trap.run();
    match result {
        Value::Error(_) => {}
        x => {
            panic!("Expected Error, got {:?}", x);
        }
    };

    let inputs = [
        Value::Null,
        Value::Atom("hello".into()),
        Value::Atom("world".into()),
        Value::Blob(0x00000000_usize.to_ne_bytes().into()),
        Value::Blob(0xcafeb0ba_usize.to_ne_bytes().into()),
        Value::Tree(vec![Value::Null, Value::Atom("1".into())].into()),
        Value::Tree(vec![].into()),
    ];

    log::info!("running identity program on {} inputs", inputs.len());
    let identity = Lambda::from_elf(IDENTITY_ELF);
    for input in inputs {
        let id = identity.clone();
        let id = id.apply(input.clone());
        let result = id.run();
        assert_eq!(input, result);
    }

    let identity = Lambda::from_elf(IDENTITY_ELF).apply(Value::Null);
    const ITERS: usize = 100;
    log::info!("running identity program {} times", ITERS);
    let time = kernel::kvmclock::time(|| {
        for _ in 0..ITERS {
            let f = identity.clone();
            let _ = core::hint::black_box(f.run());
        }
    });
    log::info!(
        "identity program takes {} ns",
        time.as_nanos() / ITERS as u128
    );

    const N: usize = 1000;
    log::info!("running add program on {} inputs", N);
    let add = Lambda::from_elf(ADD_ELF);
    for _ in 0..N {
        let i = 0;
        let x = ((10 * i + 11) % 31) as u64;
        let y = ((13 * i + 2) % 29) as u64;
        let f = add.clone();
        let fx = f.apply(Value::Blob(x.to_ne_bytes().into())).run();
        let Value::Lambda(fx) = fx else {
            panic!("add program did not produce a lambda: {:?}", fx);
        };
        let result = fx.apply(Value::Blob(y.to_ne_bytes().into())).run();
        let Value::Blob(z) = result else {
            panic!("add program did not produce a blob: {:?}", result);
        };
        let bytes: [u8; 8] = (&*z)
            .try_into()
            .expect("add program produced a blob of the wrong size");
        let z = u64::from_ne_bytes(bytes);
        assert_eq!(x + y, z);
    }

    let error = Lambda::from_elf(ERROR_ELF).apply(Value::Blob(b"hello".as_slice().into()));
    log::info!("running error program");
    let result = error.run();
    match result {
        Value::Null => {}
        x => {
            panic!("Expected Null, got {:?}", x);
        }
    };

    log::info!("done");
    shutdown();
}
