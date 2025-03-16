#![no_main]
#![no_std]
#![feature(allocator_api)]

extern crate alloc;
extern crate kernel;

use alloc::collections::vec_deque::VecDeque;
use alloc::sync::Arc;
use alloc::{collections::btree_map::BTreeMap, vec::Vec};
use core::future::Future;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::time::Duration;

use kernel::spinlock::SpinLock;
use kernel::{kvmclock, prelude::*, rt, server, tsc};
use macros::kmain;
use rand::SeedableRng;
use rand_distr::Distribution;

const INFINITE_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_infinite"));
const SPIN_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_spin"));

async fn generator(
    rate: f64,
    duration: Duration,
    done: Arc<AtomicUsize>,
    queue: Arc<SpinLock<VecDeque<Duration>>>,
) {
    let mut rng = rand::rngs::SmallRng::seed_from_u64(10);
    let exp = rand_distr::Exp::new(rate).unwrap();
    let start = kvmclock::time_since_boot();
    let mut target = start;

    while (kvmclock::time_since_boot() - start) < duration {
        // let start = tsc::read();
        let start = kvmclock::time_since_boot();

        queue.lock().push_back(start);

        let duration_secs = exp.sample(&mut rng);
        let duration = Duration::from_secs_f64(duration_secs);
        target += duration;

        while kvmclock::time_since_boot() < target {
            core::hint::spin_loop();
        }
        // rt::delay_until(target).await;
    }
    done.fetch_add(1, Ordering::SeqCst);
}

async fn serve(
    done: Arc<AtomicUsize>,
    queue: Arc<SpinLock<VecDeque<Duration>>>,
    results: Arc<SpinLock<Vec<Duration>>>,
    job: impl FnOnce() + Send + 'static + Clone,
) {
    loop {
        let Some(mut queue) = queue.try_lock() else {
            rt::yield_now().await;
            continue;
        };
        let Some(request) = queue.pop_front() else {
            core::mem::drop(queue);
            if done.load(Ordering::SeqCst) >= 1 {
                break;
            } else {
                rt::yield_now().await;
                continue;
            }
        };
        core::mem::drop(queue);
        (job.clone())();
        let now = kvmclock::time_since_boot();
        let delta = now - request;
        results.lock().push(delta);
    }
    done.fetch_add(1, Ordering::SeqCst);
}

#[kmain]
async fn kmain() {
    log::info!("kmain");

    server::SERVER.wait().run().await;

    kernel::profile::begin();

    let thunk = Arc::new(Thunk::from_elf(INFINITE_ELF));
    let job = move || {
        let thunk = (*thunk).clone();
        thunk.run_on_this_cpu();
    };

    let start = tsc::read();
    let mut i = 0;
    while tsc::read() - start < Duration::from_secs(1) {
        job();
        i += 1;
    }
    let end = tsc::read();
    let time = end - start;
    log::info!(
        "{:.2} iters/s | {:?}/iter",
        i as f64 / time.as_secs_f64(),
        time / i
    );

    let rates = [1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7];
    for rate in rates {
        let duration = Duration::from_millis(1000);
        let done = Arc::new(AtomicUsize::new(0));

        let start = kvmclock::time_since_boot();
        let queue = Arc::new(SpinLock::new(VecDeque::new()));
        let results = Arc::new(SpinLock::new(Vec::new()));

        rt::spawn(generator(rate, duration, done.clone(), queue.clone()));

        let cores = kernel::ncores();
        for _ in 0..cores {
            let job = job.clone();
            rt::spawn(serve(done.clone(), queue.clone(), results.clone(), job));
        }

        while done.load(Ordering::SeqCst) < 1 + cores {
            rt::delay_for(Duration::from_millis(1)).await;
        }
        let end = kvmclock::time_since_boot();

        let mut measurements = results.lock();

        let iters = measurements.len();
        let duration = end - start;
        measurements.sort();
        let p50 = measurements[measurements.len() / 2];
        // let p99 = measurements[measurements.len() * 99 / 100];
        // let mean =
        //     measurements.iter().fold(Duration::ZERO, |x, y| x + *y) / measurements.len() as u32;
        // let min = measurements.iter().min().unwrap();
        // let max = measurements.iter().max().unwrap();

        // log::info!("min: {min:?}");
        // log::info!("µ: {mean:?}");
        // log::info!("p50: {p50:?}");
        // log::info!("p99: {p99:?}");
        // log::info!("max: {max:?}");
        // log::info!(
        //     "{iters} requests in {duration:?} = {:.2}/s",
        //     iters as f64 / duration.as_secs_f64()
        // );
        log::info!(
            "{iters:9}: {p50:?} ({:.2}/s)",
            iters as f64 / duration.as_secs_f64()
        );
    }

    kernel::profile::end();

    profile();
}

fn profile() {
    log::info!("--- MOST FREQUENT FUNCTIONS ---");
    let entries = kernel::profile::entries();
    let entries = entries
        .into_iter()
        .map(|(p, x)| {
            (
                kernel::host::symname(p).unwrap_or_else(|| ("???".into(), 0)),
                x,
            )
        })
        .fold(BTreeMap::new(), |mut map, ((name, _offset), count)| {
            map.entry(name).and_modify(|e| *e += count).or_insert(count);
            map
        });
    let mut entries = Vec::from_iter(entries);
    entries.sort_by_key(|(_name, count)| *count);
    entries.reverse();
    for (i, &(ref name, count)) in entries.iter().take(8).enumerate() {
        log::info!("\t{i}: {count} - {name}");
    }
    log::info!("-------------------------------");
}
