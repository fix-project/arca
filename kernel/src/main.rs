#![no_main]
#![no_std]
#![feature(allocator_api)]

extern crate alloc;
extern crate kernel;

use core::time::Duration;

use alloc::{collections::btree_map::BTreeMap, vec, vec::Vec};

use kernel::{client, kvmclock, prelude::*};

const TRAP_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_trap"));
const IDENTITY_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_identity"));
const ADD_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_add"));
const ERROR_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_error"));
const CURRY_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_curry"));
const PERFORM_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_perform"));
const SPIN_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_spin"));
const INFINITE_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_infinite"));

#[no_mangle]
extern "C" fn kmain() {
    let id = kernel::coreid();

    kernel::profile::begin();

    if id == 0 {
        log::info!("kmain");
    }
    const ITERS: usize = 500;
    const N: usize = 1000;
    let mut cpu = CPU.borrow_mut();

    if id == 0 {
        client::run(&mut cpu);
    }

    // Test trap.rs
    let trap = Thunk::from_elf(TRAP_ELF);
    if id == 0 {
        log::info!("running trap program");
    }
    let result = trap.run(&mut cpu);
    match result {
        Value::Error(_) => {}
        x => {
            panic!("Expected Error, got {:?}", x);
        }
    };

    // Test identity.rs
    let inputs = [
        Value::Null,
        Value::Atom("hello".into()),
        Value::Atom("world".into()),
        Value::Blob(0x00000000_usize.to_ne_bytes().into()),
        Value::Blob(0xcafeb0ba_usize.to_ne_bytes().into()),
        Value::Tree(vec![Value::Null, Value::Atom("1".into())].into()),
        Value::Tree(vec![].into()),
    ];

    if id == 0 {
        log::info!("running identity program on {} inputs", inputs.len());
    }
    let identity = Thunk::from_elf(IDENTITY_ELF);
    let result = identity.run(&mut cpu);
    let Value::Lambda(identity) = result else {
        panic!("{result:?}");
    };
    for input in inputs {
        let id = identity.clone();
        let id = id.apply(input.clone());
        let result = id.run(&mut cpu);
        assert_eq!(input, result);
    }

    let identity = identity.apply(Value::Null);
    if id == 0 {
        log::info!("running identity program {} times", ITERS);
    }
    let time = kernel::kvmclock::time(|| {
        for _ in 0..ITERS {
            let f = identity.clone();
            let _ = core::hint::black_box(f.run(&mut cpu));
        }
    });
    if id == 0 {
        log::info!(
            "identity program takes {} ns",
            time.as_nanos() / ITERS as u128
        );
    }

    // Test add.rs
    if id == 0 {
        log::info!("running add program on {} inputs", N);
    }
    let add = Thunk::from_elf(ADD_ELF);
    let Value::Lambda(add) = add.run(&mut cpu) else {
        panic!();
    };
    for _ in 0..N {
        let i = 0;
        let x = ((10 * i + 11) % 31) as u64;
        let y = ((13 * i + 2) % 29) as u64;
        let f = add.clone();
        let f = f.load(&mut cpu);
        let result = f
            .apply(Value::Tree(
                vec![
                    Value::Blob(x.to_ne_bytes().into()),
                    Value::Blob(y.to_ne_bytes().into()),
                ]
                .into(),
            ))
            .run();
        let LoadedValue::Unloaded(Value::Blob(z)) = result else {
            panic!("add program did not produce a blob");
        };
        let bytes: [u8; 8] = (&*z)
            .try_into()
            .expect("add program produced a blob of the wrong size");
        let z = u64::from_ne_bytes(bytes);
        assert_eq!(x + y, z);
    }

    if id == 0 {
        log::info!("running add program {} times", ITERS);
    }
    let tree: Tree = vec![
        Value::Blob(100u64.to_ne_bytes().into()),
        Value::Blob(100u64.to_ne_bytes().into()),
    ]
    .into();
    let time = kernel::kvmclock::time(|| {
        for _ in 0..ITERS {
            let f = add.clone();
            let f = f.load(&mut cpu);
            let f = f.apply(Value::Tree(tree.clone()));
            let _ = core::hint::black_box(f.run());
        }
    });
    if id == 0 {
        log::info!("add program takes {} ns", time.as_nanos() / ITERS as u128);
    }

    // Test curry.rs
    if id == 0 {
        log::info!("running curried add program on {} inputs", N);
    }
    let curry = Thunk::from_elf(CURRY_ELF);
    let curry = curry.load(&mut cpu);
    let result = curry.run();
    let LoadedValue::Lambda(curry) = result else {
        panic!("expected lambda after start");
    };
    let LoadedValue::Lambda(cadd) = curry.apply(Value::Lambda(add.clone())).run() else {
        panic!("expected lambda after function");
    };
    let LoadedValue::Lambda(cadd) = cadd.apply(Value::Blob(2_u64.to_ne_bytes().into())).run()
    else {
        panic!("expected lambda after count");
    };
    let cadd = cadd.unload();
    for _ in 0..N {
        let i = 0;
        let x = ((10 * i + 11) % 31) as u64;
        let y = ((13 * i + 2) % 29) as u64;
        let f = cadd.clone();
        let f = f.load(&mut cpu);
        let LoadedValue::Lambda(fx) = f.apply(Value::Blob(x.to_ne_bytes().into())).run() else {
            panic!();
        };
        let result = fx.apply(Value::Blob(y.to_ne_bytes().into())).run();
        let LoadedValue::Unloaded(Value::Blob(z)) = result else {
            panic!("curried add program did not produce a blob");
        };
        let bytes: [u8; 8] = (&*z)
            .try_into()
            .expect("curried add program produced a blob of the wrong size");
        let z = u64::from_ne_bytes(bytes);
        assert_eq!(x + y, z);
    }

    if id == 0 {
        log::info!("running curried add program {} times", ITERS);
    }
    let time = kernel::kvmclock::time(|| {
        for _ in 0..ITERS {
            let f = cadd.clone();
            let x = 100u64;
            let y = 100u64;
            let f = f.load(&mut cpu);
            let LoadedValue::Lambda(fx) = f.apply(Value::Blob(x.to_ne_bytes().into())).run() else {
                panic!();
            };
            let result = fx.apply(Value::Blob(y.to_ne_bytes().into())).run();
            let LoadedValue::Unloaded(Value::Blob(z)) = result else {
                panic!("curried add program did not produce a blob: {:?}", result);
            };
            let bytes: [u8; 8] = (&*z)
                .try_into()
                .expect("curried add program produced a blob of the wrong size");
            let z = u64::from_ne_bytes(bytes);
            assert_eq!(x + y, z);
        }
    });
    if id == 0 {
        log::info!(
            "curried add program takes {} ns",
            time.as_nanos() / ITERS as u128
        );
    }

    // Test error.rs
    let error = Thunk::from_elf(ERROR_ELF);
    let Value::Lambda(error) = error.run(&mut cpu) else {
        panic!();
    };
    let error = error.apply(Value::Blob(b"hello".as_slice().into()));
    if id == 0 {
        log::info!("running error program");
    }
    let result = error.run(&mut cpu);
    match result {
        Value::Null => {}
        x => {
            panic!("Expected Null, got {:?}", x);
        }
    };

    // Test perform.rs
    let inputs = [
        Value::Null,
        Value::Atom("hello".into()),
        Value::Atom("world".into()),
        Value::Blob(0x00000000_usize.to_ne_bytes().into()),
        Value::Blob(0xcafeb0ba_usize.to_ne_bytes().into()),
        Value::Tree(vec![Value::Null, Value::Atom("1".into())].into()),
        Value::Tree(vec![].into()),
    ];

    if id == 0 {
        log::info!("running perform program on {} inputs", inputs.len());
    }

    let perform = Thunk::from_elf(PERFORM_ELF);
    let result = perform.run(&mut cpu);
    let Value::Tree(perform) = result else {
        panic!("{result:?}");
    };

    let expected_payload = "effect".as_bytes();
    let Value::Blob(perform_payload) = perform[0].clone() else {
        panic!("{perform:?}");
    };
    if perform_payload.as_ref() != expected_payload {
        panic!("{perform:?}");
    }

    let Value::Lambda(perform) = perform[1].clone() else {
        panic!("{perform:?}");
    };
    for input in inputs {
        let id = perform.clone();
        let id = id.apply(input.clone());
        let result = id.run(&mut cpu);
        assert_eq!(input, result);
    }

    // Test spin.rs
    const DURATION: Duration = Duration::from_millis(100);
    if id == 0 {
        log::info!("running spin program for {:?}", DURATION);
    }
    let spin = Thunk::from_elf(SPIN_ELF);
    let mut spin = spin.load(&mut cpu);
    let mut now = kvmclock::now();
    let end = now + DURATION;
    let mut i = 0;
    while now < end {
        let LoadedValue::Thunk(x) = spin.run_for(Duration::from_millis(1)) else {
            panic!("expected spin program to time out");
        };
        spin = x;
        now = kvmclock::now();
        i += 1;
    }
    spin.unload();
    if id == 0 {
        log::info!("done after {i} iterations");
    }

    let infinite = Thunk::from_elf(INFINITE_ELF);
    if id == 0 {
        log::info!("running infinite program for {} iterations", ITERS);
    }
    let mut infinite = infinite.load(&mut cpu);
    let time = kernel::kvmclock::time(|| {
        for _ in 0..ITERS {
            let LoadedValue::Lambda(y) = infinite.run() else {
                panic!();
            };
            infinite = y.apply(Value::Null);
        }
        infinite.unload();
    });
    if id == 0 {
        log::info!(
            "infinite program takes {} ns",
            time.as_nanos() / ITERS as u128
        );
    }
    if id == 0 {
        log::info!(
            "running infinite program with unloads for {} iterations",
            ITERS
        );
    }
    let mut infinite = Thunk::from_elf(INFINITE_ELF);
    let time = kernel::kvmclock::time(|| {
        for _ in 0..ITERS {
            let loaded = infinite.load(&mut cpu);
            let Value::Lambda(y) = loaded.run().unload() else {
                panic!();
            };
            infinite = y.apply(Value::Null);
        }
    });
    if id == 0 {
        log::info!(
            "infinite program takes {} ns",
            time.as_nanos() / ITERS as u128
        );
    }

    kernel::profile::end();

    if id == 0 {
        log::info!("most frequent functions:");
        let entries = kernel::profile::entries();
        let entries = entries
            .into_iter()
            .map(|(p, x)| {
                (
                    kernel::host::symname(p).unwrap_or_else(|| ("???".into(), 0)),
                    x,
                )
            })
            .fold(BTreeMap::new(), |mut map, ((name, _offset), count)| {
                map.entry(name).and_modify(|e| *e += count).or_insert(count);
                map
            });
        let mut entries = Vec::from_iter(entries);
        entries.sort_by_key(|(_name, count)| *count);
        entries.reverse();
        for (i, &(ref name, count)) in entries[..16].iter().enumerate() {
            log::info!("\t{i}: {count} - {name}");
        }
    }
}
