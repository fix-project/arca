#![feature(allocator_api)]

use std::num::NonZero;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use kvm_ioctls::Kvm;
use vmm::runtime::Mmap;
use vmm::runtime::Runtime;

use tokio::task::JoinSet;

const INFINITE_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_infinite"));

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tokio::spawn(async {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

        let kvm = Box::leak(Kvm::new().unwrap().into());
        let mmap = Box::leak(Mmap::new(1 << 32).into());
        let smp: usize = std::thread::available_parallelism()
            .unwrap_or(NonZero::new(1).unwrap())
            .into();
        let runtime = Box::leak(Runtime::new(kvm, mmap, smp).into());
        log::info!("creating client");
        let iters = 10000;
        let measurements = Arc::new(Mutex::new(vec![]));
        let mut set = JoinSet::new();
        let client = runtime.client();
        let inf = Arc::new(client.blob(INFINITE_ELF).await.unwrap());
        for _ in 0..iters {
            let m = measurements.clone();
            let inf = inf.clone();
            let inf = inf.duplicate().await.unwrap();
            set.spawn(async move {
                let start = std::time::SystemTime::now();
                let inf = inf.create_thunk().await.unwrap();
                inf.run().await.unwrap();
                let end = std::time::SystemTime::now();
                let duration = end.duration_since(start).unwrap();
                let mut measurements = m.lock().unwrap();
                measurements.push(duration);
            });
        }
        let start = std::time::SystemTime::now();
        set.join_all().await;
        let end = std::time::SystemTime::now();
        let duration = end.duration_since(start).unwrap();
        let time = duration / iters;

        let mut measurements = measurements.lock().unwrap();
        measurements.sort();
        let p99 = measurements[measurements.len()*99/100];
        dbg!(measurements.len());
        let mean =
            measurements.iter().fold(Duration::ZERO, |x, y| x + *y) / measurements.len() as u32;
        let min = measurements.iter().min().unwrap();
        let max = measurements.iter().max().unwrap();

        log::info!("min: {min:?}");
        log::info!("µ: {mean:?}");
        log::info!("p99: {p99:?}");
        log::info!("max: {max:?}");
        log::info!("time: {time:?}");
        log::info!("{iters} requests in {duration:?} = {:.0}/s", iters as f64 / duration.as_secs_f64());
        Ok(())
    }).await?
}
