#![feature(allocator_api)]

use std::num::NonZero;

use kvm_ioctls::Kvm;
use vmm::runtime::Mmap;
use vmm::runtime::Runtime;

// const ADD_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_add"));
// const INFINITE_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_infinite"));
// const KERNEL_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_KERNEL_kernel"));

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let kvm = Kvm::new().unwrap();
    let mut mmap = Mmap::new(1 << 32);
    let smp = std::thread::available_parallelism()
        .unwrap_or(NonZero::new(1).unwrap())
        .into();
    let runtime = Runtime::new(&kvm, &mut mmap, smp);
    log::info!("creating client");
    let client = runtime.client();
    let port = client.port();
    let iters = 100000;
    let start = std::time::SystemTime::now();
    let y = port.word(0xcafeb0ba);
    for _ in 0..iters {
        port.word(0xcafeb0ba);
    }
    let end = std::time::SystemTime::now();
    log::info!(
        "{:#x} - time={:?}",
        y.read(),
        end.duration_since(start).unwrap() / iters
    );
    Ok(())
}
