#![feature(allocator_api)]
#![feature(thread_sleep_until)]
#![feature(future_join)]

use std::num::NonZero;

use vmm::runtime::Runtime;

const ARCADE: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_ARCADE_arcade"));

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let smp = std::thread::available_parallelism()
        .unwrap_or(NonZero::new(1).unwrap())
        .get();
    let mut runtime = Runtime::new(smp, 1 << 30, ARCADE.into());

    runtime.run(&[]);

    Ok(())
}
