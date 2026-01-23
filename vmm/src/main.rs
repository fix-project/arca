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
mod relay;
mod tcp;
mod vsock;

use ::vsock::{VsockAddr, VsockListener};
use fs::*;
use tcp::*;
use vsock::*;

#[allow(unused)]
use crate::relay::relay_tcp_vsock;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let max_smp = std::thread::available_parallelism()
        .unwrap_or(NonZero::new(1).unwrap())
        .get();

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
                .short('s')
                .long("smp")
                .help("Number of CPU cores for the guest VM")
                .value_parser(clap::value_parser!(usize))
                .required(false),
        )
        .arg(
            Arg::new("cid")
                .short('c')
                .long("cid")
                .help("Guest VM's CID")
                .value_parser(clap::value_parser!(usize))
                .default_value("3")
                .required(false),
        )
        .get_matches();

    let fix = matches.get_flag("fix");

    // let (_soft, hard) = rlimit::getrlimit(rlimit::Resource::AS).unwrap();
    // rlimit::setrlimit(rlimit::Resource::AS, hard, hard).unwrap();
    // log::info!("set max address space size to {hard} bytes");

    let smp = *matches.get_one::<usize>("smp").unwrap_or(&max_smp);
    let cid = *matches.get_one::<usize>("cid").unwrap();

    let host_listener_port = 1234;

    let mut runtime = if fix {
        Runtime::new(cid, smp, 1 << 34, FIX.into())
    } else {
        Runtime::new(cid, smp, 1 << 34, ARCADE.into())
    };

    std::thread::spawn(move || {
        let spawn = move |x| {
            smol::spawn(x).detach();
        };

        let sock =
            VsockListener::bind(&VsockAddr::new(VMADDR_CID_HOST, host_listener_port)).unwrap();

        let mut s = ninep::Server::new(spawn);
        let dir = FsDir::new(".", Open::ReadWrite).unwrap();
        let home = FsDir::new(std::env::home_dir().unwrap(), Open::ReadWrite).unwrap();
        let tcp = TcpFS::default();
        smol::block_on(async {
            s.add("", dir).await;
            s.add("home", home).await;
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

    if fix {
        runtime.run(&[]);
    } else {
        let bin = std::fs::read(matches.get_one::<std::path::PathBuf>("bin").unwrap())?;

        let mut rtbin = Vec::new_in(BuddyAllocator);
        rtbin.extend_from_slice(&bin);
        let rtbin = rtbin.into_boxed_slice();
        let ptr = BuddyAllocator.to_offset(rtbin.as_ptr());
        let len = rtbin.len();
        Box::leak(rtbin);
        runtime.run(&[ptr, len]);
    }

    Ok(())
}
