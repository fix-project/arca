#![feature(allocator_api)]
#![feature(thread_sleep_until)]
#![feature(future_join)]

use std::num::NonZero;
use std::sync::LazyLock;
use std::time::Duration;
use std::time::Instant;

use futures::stream::FuturesUnordered;
use futures::StreamExt;
use kvm_ioctls::Kvm;
use vmm::client::Client;
use vmm::runtime::Mmap;
use vmm::runtime::Runtime;

const INFINITE_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_infinite"));

static SMP: LazyLock<usize> = LazyLock::new(|| {
    std::thread::available_parallelism()
        .unwrap_or(NonZero::new(1).unwrap())
        .into()
});

static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    let kvm = Box::leak(Kvm::new().unwrap().into());
    let mmap = Box::leak(Mmap::new(1 << 32).into());
    Runtime::new(kvm, mmap, *SMP)
});
static CLIENT: LazyLock<&Client> = LazyLock::new(|| RUNTIME.client());

async fn test(end: Instant) -> usize {
    let client = &*CLIENT;
    let elf = client.blob(INFINITE_ELF).await;
    let thunk = elf.create_thunk().await;
    let mut iters = 0;
    while Instant::now() < end {
        let thunk = thunk.duplicate().await;
        let _ = thunk.run().await;
        iters += 1;
    }
    iters
}

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let runtime = &*RUNTIME;

    let cores = *SMP;
    let lg_cores = cores.ilog2();

    let duration = Duration::from_millis(500);
    for lg_n in 0..(lg_cores + 2) {
        let n = 1 << lg_n;
        let set = FuturesUnordered::new();
        let now = Instant::now();
        for _ in 0..n {
            set.push(test(now + duration));
        }
        let results: Vec<_> = set.collect().await;
        let iters: usize = results.iter().sum();
        let time = now.elapsed() / iters as u32;
        log::info!("{n:2} cores: {time:?} per iteration ({} total)", iters);
    }

    runtime.shutdown();
    Ok(())
}
