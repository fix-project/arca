#![feature(allocator_api)]
#![feature(thread_sleep_until)]
#![feature(future_join)]

use std::{num::NonZero, path::PathBuf, sync::Arc};

use clap::{Arg, ArgAction, Command};
use common::BuddyAllocator;
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
        .arg(
            Arg::new("bin")
                .value_parser(clap::value_parser!(PathBuf))
                .conflicts_with("fix")
                .required(true),
        )
        .arg(
            Arg::new("smp")
                .long("smp")
                .value_parser(clap::value_parser!(usize))
        )
        .get_matches();

    let run_fix = matches.get_flag("fix");

    let smp = matches.get_one("smp").cloned().unwrap_or_else(
        || if run_fix {
            1
        } else {
            std::thread::available_parallelism()
                .unwrap_or(NonZero::new(1).unwrap())
                .get()
        });

    let (_soft, hard) = rlimit::getrlimit(rlimit::Resource::AS).unwrap();
    rlimit::setrlimit(rlimit::Resource::AS, hard, hard).unwrap();
    log::info!("set max address space size to {hard} bytes");

    // let smp = core::cmp::min(smp, 1);
    let mut runtime = if run_fix {
        Runtime::new(smp, 1 << 32, FIX.into())
    } else {
        Runtime::new(smp, 1 << 32, ARCADE.into())
    };
    let bin = matches
        .get_one::<PathBuf>("bin")
        .expect("bin argument is required; clap should have caught this");

    let bin = std::fs::read(bin)?;

    std::thread::spawn(|| {
        let spawn = move |x| {
            smol::spawn(x).detach();
        };

        let sock = VsockListener::bind(&VsockAddr::new(VMADDR_CID_HOST, 1564)).unwrap();

        let mut s = ninep::Server::new(spawn);
        let dir = FsDir::new("/tmp", Open::ReadWrite).unwrap();
        let tcp = TcpFS::default();
        smol::block_on(async {
            s.add("", dir).await;
            s.add("tcp", tcp).await;
        });
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

    let mut rtbin = Vec::new_in(BuddyAllocator);
    rtbin.extend_from_slice(&bin);
    let rtbin = rtbin.into_boxed_slice();
    let ptr = BuddyAllocator.to_offset(rtbin.as_ptr());
    let len = rtbin.len();
    Box::leak(rtbin);

    runtime.run(&[ptr, len]);

    Ok(())
}
