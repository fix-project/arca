#![feature(allocator_api)]

use std::num::NonZero;
use std::time::Duration;

use kvm_ioctls::Kvm;
use vmm::client::ArcaRef;
use vmm::runtime::Mmap;
use vmm::runtime::Runtime;

// const ADD_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_add"));
const INFINITE_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_infinite"));
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
    log::info!("sleeping");
    std::thread::sleep(Duration::from_millis(100));
    let iters = 100000;
    log::info!("creating blob");
    let inf = client.create_blob(INFINITE_ELF);
    let inf = inf.into_thunk()?;
    let ArcaRef::Lambda(mut inf) = inf.run()? else {
        panic!();
    };
    let start = std::time::SystemTime::now();
    for _ in 0..iters {
        let ArcaRef::Lambda(result) = inf.apply_and_run(client.null().into())? else {
            panic!();
        };
        inf = result;
    }
    let end = std::time::SystemTime::now();
    log::info!(
        "running infinite program took {:?}",
        end.duration_since(start).unwrap() / iters
    );
    Ok(())
}
