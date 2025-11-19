#![feature(allocator_api)]
#![feature(thread_sleep_until)]
#![feature(future_join)]

use std::{num::NonZero, sync::Arc};

use clap::{Arg, Command};
use libc::VMADDR_CID_HOST;
use ninep::*;
use vfs::Open;
use vmm::runtime::Runtime;

const ARCADE: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_ARCADE_arcade"));
const FIX: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_FIX_fix"));
const WASI: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_WASI_wasi"));

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
        .arg(
            Arg::new("BIN")
                .long("bin")
                .help("binary to run")
                .value_parser(["arcade", "fix", "wasi"])
                .default_value("arcade"),
        )
        .get_matches();

    let bin: &String = matches.get_one("BIN").expect("default");

    let smp = match bin.as_str() {
        "arcade" => std::thread::available_parallelism()
            .unwrap_or(NonZero::new(1).unwrap())
            .get(),
        "fix" => 1,
        "wasi" => 1,
        _ => panic!("value is not allowed"),
    };

    let mut runtime = match bin.as_str() {
        "arcade" => Runtime::new(smp, 1 << 30, ARCADE.into()),
        "fix" => Runtime::new(smp, 1 << 30, FIX.into()),
        "wasi" => Runtime::new(smp, 1 << 30, WASI.into()),
        _ => panic!("value is not allowed"),
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
