#![feature(allocator_api)]
#![feature(thread_sleep_until)]
#![feature(future_join)]

use std::{num::NonZero, sync::Arc};

use clap::{Arg, ArgAction, Command};
use libc::VMADDR_CID_HOST;
use ninep::*;
use vfs::Open;
use vmm::runtime::Runtime;

const ARCADE: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_ARCADE_arcade"));
const FIX: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_FIX_fix"));

mod fs;
mod tcp;
mod vsock;

use ::vsock::{VsockAddr, VsockListener};
use fs::*;
use tcp::*;
use vsock::*;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let matches = Command::new("arca-vmm")
        .arg(Arg::new("fix").long("fix").action(ArgAction::SetTrue))
        .get_matches();

    let run_fix = matches.get_flag("fix");

    let smp = if run_fix {
        1
    } else {
        std::thread::available_parallelism()
            .unwrap_or(NonZero::new(1).unwrap())
            .get()
    };
    // let smp = core::cmp::min(smp, 1);
    let mut runtime = if run_fix {
        Runtime::new(smp, 1 << 30, FIX.into())
    } else {
        Runtime::new(smp, 1 << 30, ARCADE.into())
    };

    std::thread::spawn(|| {
        let spawn = move |x| {
            smol::spawn(x).detach();
        };

        let sock = VsockListener::bind(&VsockAddr::new(VMADDR_CID_HOST, 1564)).unwrap();

        let mut s = ninep::Server::new(spawn);
        let dir = FsDir::new("/tmp", Open::ReadWrite).unwrap();
        s.add_blocking("", dir);
        let tcp = TcpFS::default();
        s.add_blocking("tcp", tcp);
        let s = Arc::new(s);

        loop {
            let (st, _) = sock.accept().unwrap();
            let s = s.clone();
            smol::spawn(async move {
                let vsock = Vsock::new(st);
                s.serve(vsock).await.unwrap();
            })
            .detach();
        }
    });

    runtime.run(&[]);

    Ok(())
}
