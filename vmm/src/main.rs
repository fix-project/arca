#![feature(allocator_api)]

use std::num::NonZero;

use kvm_ioctls::Kvm;
use vmm::runtime::Mmap;
use vmm::runtime::Runtime;

const INFINITE_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_infinite"));

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let kvm = Kvm::new().unwrap();
    let mut mmap = Mmap::new(1 << 32);
    let smp: usize = std::thread::available_parallelism()
        .unwrap_or(NonZero::new(1).unwrap())
        .into();
    let runtime = Runtime::new(&kvm, &mut mmap, smp);
    log::info!("creating client");
    let client = runtime.client();
    let iters = 100000;
    let start = std::time::SystemTime::now();
    let inf = client.blob(INFINITE_ELF)?;
    for _ in 0..iters {
        let inf = inf.clone();
        let inf = inf.create_thunk()?;
        inf.run()?;
    }
    let end = std::time::SystemTime::now();
    log::info!("time={:?}", end.duration_since(start).unwrap() / iters);
    Ok(())
}
