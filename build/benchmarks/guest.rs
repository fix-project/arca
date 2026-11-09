#![no_std]

use benchmark_protocol::{READY, REQUEST_LEN};
use common::protocol::control::StandardStream;
use core::hint::black_box;
use core::sync::atomic::{Ordering, compiler_fence};
use core::time::Duration;
use kernel::host::fs::File;
use kernel::prelude::*;

pub use kernel::testing::fixture;

pub trait Benchmark {
    fn run(&mut self, iterations: u64) -> Duration;
}

pub fn run<B: Benchmark>(create: impl Fn(usize, usize) -> B) -> ! {
    let args = kernel::host::os::argv();
    for entry in args[1..].as_chunks::<3>().0 {
        assert_eq!(entry[0], "--fixture");
        fixture(&entry[1]);
    }
    let mut input = File::stdio(StandardStream::Stdin).unwrap();
    let mut output = File::stdio(StandardStream::Stdout).unwrap();
    output.try_write_all(&READY).unwrap();
    let mut request = kernel::allocator::buffer(vec![0; REQUEST_LEN]);
    let mut benchmark = None;
    while input.read_exact(&mut request) == REQUEST_LEN {
        let operation = request[0] as usize;
        let size = u64::from_le_bytes(request[1..9].try_into().unwrap()) as usize;
        let iterations = u64::from_le_bytes(request[9..17].try_into().unwrap());
        let nanos = if iterations == 0 {
            benchmark = Some(create(operation, size));
            0
        } else {
            let elapsed = benchmark.as_mut().unwrap().run(iterations);
            u64::try_from(elapsed.as_nanos()).unwrap()
        };
        output.try_write_all(&nanos.to_le_bytes()).unwrap();
    }
    kernel::shutdown();
}

pub fn timed<T>(f: impl FnOnce() -> T) -> (Duration, T) {
    compiler_fence(Ordering::SeqCst);
    let start = kernel::tsc::read_cycles();
    let output = black_box(f());
    compiler_fence(Ordering::SeqCst);
    unsafe { core::arch::x86_64::_mm_mfence() };
    let end = kernel::tsc::read_cycles();
    (kernel::tsc::cycles_to_duration(end - start), output)
}

pub fn state_batch(size: usize) -> usize {
    ((16 << 20) / (size + 4096)).clamp(1, 64)
}

pub fn batches(
    mut iterations: u64,
    limit: usize,
    mut operation: impl FnMut(usize) -> Duration,
) -> Duration {
    let mut elapsed = Duration::ZERO;
    while iterations != 0 {
        let count = iterations.min(limit as u64) as usize;
        elapsed += operation(count);
        iterations -= count as u64;
    }
    elapsed
}
