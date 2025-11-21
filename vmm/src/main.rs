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
use common::ipaddr::IpAddr;
use fs::*;
use tcp::*;
use vsock::*;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let max_smp = std::thread::available_parallelism()
        .unwrap_or(NonZero::new(1).unwrap())
        .get();

    let matches = Command::new("arca-vmm")
        .arg(Arg::new("fix").long("fix").action(ArgAction::SetTrue))
        .arg(
            Arg::new("smp")
                .short('s')
                .long("smp")
                .help("Number of CPU cores for the guest VM")
                .required(false),
        )
        .arg(
            Arg::new("cid")
                .short('c')
                .long("cid")
                .help("Guest VM's CID")
                .default_value("3")
                .required(false),
        )
        .arg(
            Arg::new("host")
                .long("host")
                .help("IP address/port number for TCP listener in the guest VM")
                .default_value("127.0.0.1:11211")
                .required(false),
        )
        .arg(
            Arg::new("listener")
                .short('l')
                .long("is-listener")
                .help("Run as arca listening for continuations")
                .action(ArgAction::SetTrue)
                .required(false),
        )
        .get_matches();

    let run_fix = matches.get_flag("fix");

    let smp = matches
        .get_one::<String>("smp")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(max_smp);

    let cid = matches
        .get_one::<String>("cid")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(3);

    let host = matches.get_one::<String>("host").unwrap();

    let is_listener = matches.get_flag("listener");

    // TODO(kmohr): can this create an invalid port number?
    let host_listener_port = (cid as u32) + 1561;

    let mut runtime = if run_fix {
        Runtime::new(cid, smp, 1 << 32, FIX.into())
    } else {
        Runtime::new(cid, smp, 1 << 32, ARCADE.into())
    };

    std::thread::spawn(move || {
        let spawn = move |x| {
            smol::spawn(x).detach();
        };

        let sock =
            VsockListener::bind(&VsockAddr::new(VMADDR_CID_HOST, host_listener_port)).unwrap();

        let mut s = ninep::Server::new(spawn);
        let dir = FsDir::new("/tmp", Open::ReadWrite).unwrap();
        s.add_blocking("", dir);
        let tcp = TcpFS::default();
        s.add_blocking("tcp", tcp);
        // put all data for the 9P server to read/write in ~/data
        let shared_data_dir = FsDir::new("/home/kmohr/data", Open::ReadWrite).unwrap();
        s.add_blocking("data", shared_data_dir);
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

    let ipaddr = IpAddr::try_from(host.as_str()).unwrap();
    log::info!("Guest VM will connect to host at {}", ipaddr.port);
    let ipaddr: u64 = u64::from(IpAddr::try_from(host.as_str()).unwrap());

    log::info!(
        "Running {} on VM cid={} and hostname {} with {} core(s)",
        if run_fix { "fix" } else { "arcade" },
        cid,
        host,
        smp
    );

    // XXX: this will break if usize is smaller than u64
    runtime.run(&[
        cid,
        host_listener_port as usize,
        ipaddr as usize,
        is_listener as usize,
    ]);

    Ok(())
}
