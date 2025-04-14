#![feature(allocator_api)]
#![feature(thread_sleep_until)]
#![feature(future_join)]

use std::num::NonZero;

use vmm::runtime::Mmap;
use vmm::runtime::Runtime;

const KERNEL_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_KERNEL_kernel"));

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let smp = std::thread::available_parallelism()
        .unwrap_or(NonZero::new(1).unwrap())
        .get();
    let mmap = Box::leak(Mmap::new(1 << 35).into());
    let runtime = Runtime::new(smp, mmap, KERNEL_ELF.into());
    runtime.run(&[]);

    log::info!("shutting down");
    Ok(())
}
