#![feature(allocator_api)]
#![feature(thread_sleep_until)]
#![feature(future_join)]

use std::{num::NonZero, sync::Arc};

use libc::VMADDR_CID_HOST;
use ninep::*;
use tokio_vsock::{VsockAddr, VsockListener};
use vfs::Open;
use vmm::runtime::Runtime;

const ARCADE: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_ARCADE_arcade"));

mod fs;
mod tcp;
mod vsock;

use fs::*;
use tcp::*;
use vsock::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let smp = std::thread::available_parallelism()
        .unwrap_or(NonZero::new(1).unwrap())
        .get();
    let mut runtime = Runtime::new(smp, 1 << 30, ARCADE.into());

    tokio::spawn(async {
        let sock = VsockListener::bind(VsockAddr::new(VMADDR_CID_HOST, 1564)).unwrap();
        let mut s = ninep::Server::new();
        let dir = FsDir::new("/tmp", Open::ReadWrite).unwrap();
        s.add("", dir);
        let tcp = TcpFS::default();
        s.add("tcp", tcp);
        let s = Arc::new(s);

        loop {
            let (st, _) = sock.accept().await.unwrap();
            let s = s.clone();
            tokio::spawn(async move {
                let vsock = Vsock::new(st);
                s.serve(vsock).await.unwrap();
            });
        }
    });

    tokio::task::spawn_blocking(move || {
        runtime.run(&[]);
    })
    .await?;

    Ok(())
}
