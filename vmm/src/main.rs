#![feature(allocator_api)]
#![feature(thread_sleep_until)]
#![feature(future_join)]

use std::num::NonZero;

use vmm::runtime::Mmap;
use vmm::runtime::Runtime;

const KERNEL_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_KERNEL_kernel"));

// const ADD_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_add"));

// static CLIENT: LazyLock<&Client> = LazyLock::new(|| RUNTIME.client());

/*
async fn test(end: Instant) -> usize {
    let client = &*CLIENT;
    let elf = client.blob(ADD_ELF).await;
    let thunk = elf.create_thunk().await;
    let Ok(lambda) = thunk.run().await.as_lambda().await else {
        panic!();
    };
    let mut iters = 0;
    while Instant::now() < end {
        let lambda = lambda.duplicate().await;
        let args = client
            .tree([client.word(1).await.into(), client.word(2).await.into()])
            .await;
        let thunk = lambda.apply(args.into()).await;
        let _ = thunk.run().await;
        iters += 1;
    }
    iters
}
*/

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let smp = std::thread::available_parallelism()
        .unwrap_or(NonZero::new(1).unwrap())
        .get();
    let mmap = Box::leak(Mmap::new(1 << 32).into());
    let mut runtime = Runtime::new(smp, mmap);
    runtime.run(KERNEL_ELF, &[]);

    /*
    let cores = *SMP;
    let lg_cores = cores.ilog2();

    let duration = Duration::from_millis(100);
    let options = 0..(lg_cores + 3);
    for lg_n in options {
        let n = 1 << lg_n;
        let now = Instant::now();
        let mut set = vec![];
        for _ in 0..n {
            set.push(async_std::task::spawn(test(now + duration)));
        }
        let mut results = vec![];
        for x in set {
            results.push(x.await);
        }
        let mut iters: usize = results.iter().sum();
        if iters == 0 {
            iters = 1;
        }
        let elapsed = now.elapsed();
        let time = elapsed / iters as u32;
        let iters_per_second = iters as f64 / elapsed.as_secs_f64();
        log::info!("{n:4} threads: {time:?} per iteration ({iters_per_second} iters/second)");
    }
    */

    log::info!("shutting down");
    Ok(())
}
