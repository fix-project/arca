#![feature(allocator_api)]
#![feature(thread_sleep_until)]

use std::num::NonZero;
// use std::time::{Duration, Instant};
// use std::future::Future;

use kvm_ioctls::Kvm;
use vmm::runtime::Mmap;
use vmm::runtime::Runtime;

// use rand_distr::Distribution;

// const INFINITE_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_infinite"));

fn main() -> anyhow::Result<()> {
    smol::block_on(async {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

        let kvm = Box::leak(Kvm::new().unwrap().into());
        let mmap = Box::leak(Mmap::new(1 << 32).into());
        let smp: usize = std::thread::available_parallelism()
            .unwrap_or(NonZero::new(1).unwrap())
            .into();
        let runtime = Box::leak(Runtime::new(kvm, mmap, smp).into());
        log::info!("creating client");
        // let mut measurements = vec![];
        // let client = Box::leak(runtime.client().into());

        // let (tx, rx) = smol::channel::unbounded();
        // let duration = Duration::from_secs(1);
        // let start = Instant::now();
        // generate_requests(100000., duration, tx, async || {
        //     let inf = client.blob(INFINITE_ELF).await.unwrap();
        //     let inf = inf.duplicate().await.unwrap();
        //     let inf = inf.create_thunk().await.unwrap();
        //     inf.run().await.unwrap();
        // }).await;

        // while let Ok(x) = rx.recv().await {
        //     measurements.push(x);
        // }
        // let duration = start.elapsed();
        // let iters = measurements.len();

        // let time = duration/iters as u32;

        // measurements.sort();
        // let p99 = measurements[measurements.len()*99/100];
        // dbg!(measurements.len());
        // let mean =
        //     measurements.iter().fold(Duration::ZERO, |x, y| x + *y) / measurements.len() as u32;
        // let min = measurements.iter().min().unwrap();
        // let max = measurements.iter().max().unwrap();

        // log::info!("min: {min:?}");
        // log::info!("µ: {mean:?}");
        // log::info!("p99: {p99:?}");
        // log::info!("max: {max:?}");
        // log::info!("time: {time:?}");
        // log::info!("{iters} requests in {duration:?} = {:.2}/s", iters as f64 / duration.as_secs_f64());

        runtime.shutdown();
        Ok(())
    })
}

// async fn generate_requests<Fut: Future<Output=()> + Send>(rate: f64, duration: Duration, tx: smol::channel::Sender<Duration>, f: impl Fn() -> Fut + Send + 'static + Clone + Sync) {

//     let (tx2, rx2) = smol::channel::unbounded();
//     let spawner = blocking::unblock(move || {
//         let mut rng = rand::rng();
//         let exp = rand_distr::Exp::new(rate).unwrap();
//         let start = Instant::now();
//         let mut last = start;

//         while start.elapsed() < duration {
//             let now = Instant::now();
//             tx2.send_blocking(now).unwrap();
//             let duration_secs = exp.sample(&mut rng);
//             let duration = Duration::from_secs_f64(duration_secs);
//             last += duration;
//             std::thread::sleep_until(last);
//         }
//     });

//     while let Ok(start) = rx2.recv().await {
//         let start = Instant::now();
//         f().await;
//         let time = start.elapsed();
//         tx.send(time).await.unwrap();
//     }

//     spawner.await;
// }
