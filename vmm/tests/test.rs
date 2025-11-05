#![feature(allocator_api)]
#![feature(thread_sleep_until)]
#![feature(future_join)]

use vmm::runtime::Runtime;

const ARCADE: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_ARCADE_arcade"));

#[test]
fn test_arcade() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let smp = 1;
    let mut runtime = Runtime::new(smp, 1 << 30, ARCADE.into());
    runtime.run(&[]);
}
