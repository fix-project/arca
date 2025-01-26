#![no_main]
#![no_std]
#![feature(allocator_api)]

extern crate alloc;
extern crate kernel;

use alloc::vec;

use kernel::{prelude::*, shutdown};

const TRAP_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_trap"));
const IDENTITY_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_identity"));
const INC_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_inc"));

#[no_mangle]
#[inline(never)]
extern "C" fn kmain() -> ! {
    log::info!("kmain");

    let trap = Lambda::from_elf(TRAP_ELF).apply(Value::None);
    log::info!("running trap program");
    let result = trap.run();
    match result {
        Value::Error(_) => {}
        x => {
            panic!("Expected Error, got {:?}", x);
        }
    };

    let inputs = [
        Value::None,
        Value::Atom("hello".into()),
        Value::Atom("world".into()),
        Value::Blob(0x00000000_usize.to_ne_bytes().into()),
        Value::Blob(0xcafeb0ba_usize.to_ne_bytes().into()),
        Value::Tree(vec![Value::None, Value::Atom("1".into())].into()),
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

    let identity = Lambda::from_elf(IDENTITY_ELF).apply(Value::None);
    const ITERS: usize = 1000;
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
    log::info!("running increment program on {} inputs", N);
    let inc = Lambda::from_elf(INC_ELF);
    for i in 0..N {
        let f = inc.clone();
        let result = f.apply(Value::Blob((i as u64).to_ne_bytes().into())).run();
        let Value::Blob(x) = result else {
            panic!("increment program did not produce a blob");
        };
        let bytes: [u8; 8] = (&*x)
            .try_into()
            .expect("increment program produced a blob of the wrong size");
        let j = u64::from_ne_bytes(bytes);
        assert_eq!(i as u64 + 1, j);
    }

    log::info!("done");
    shutdown();
}
