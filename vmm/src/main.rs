#![feature(allocator_api)]

use std::num::NonZero;
use std::sync::Arc;

use kvm_ioctls::Kvm;
use vmm::runtime::Mmap;
use vmm::runtime::Runtime;

use tokio::task::JoinSet;

const INFINITE_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_infinite"));

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let kvm = Box::leak(Kvm::new().unwrap().into());
    let mmap = Box::leak(Mmap::new(1 << 32).into());
    let smp: usize = std::thread::available_parallelism()
        .unwrap_or(NonZero::new(1).unwrap())
        .into();
    let runtime = Arc::new(Runtime::new(kvm, mmap, smp));
    log::info!("creating client");
    let iters = 10000;
    let mut set = JoinSet::new();
    for _ in 0..smp {
        let rt = runtime.clone();
        set.spawn(async move {
            let client = rt.client();
            let inf = client.blob(INFINITE_ELF).await.unwrap();
            for _ in 0..iters {
                let inf = inf.duplicate().await.unwrap();
                let inf = inf.create_thunk().await.unwrap();
                inf.run().await.unwrap();
            }
        });
    }
    let start = std::time::SystemTime::now();
    set.join_all().await;
    let end = std::time::SystemTime::now();
    log::info!(
        "time={:?}",
        end.duration_since(start).unwrap() / (smp * iters) as u32
    );
    Ok(())
}
