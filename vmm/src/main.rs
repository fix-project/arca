#![feature(allocator_api)]
#![feature(thread_sleep_until)]
#![feature(future_join)]

use std::{num::NonZero, sync::Arc};

use clap::{Arg, ArgAction, Command};
use common::ipaddr::IpAddr;
use libc::VMADDR_CID_HOST;
use ninep::*;
use std::net::{TcpListener, TcpStream};
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
        .arg(
            Arg::new("iam")
                .long("iam")
                .help("Self IP address")
                .default_value("127.0.0.1:11211")
                .required(false),
        )
        .arg(
            Arg::new("peer")
                .long("peer")
                .help("Peer IP address")
                .default_value("127.0.0.1:11212")
                .required(false),
        )
        .arg(
            Arg::new("listener")
                .short('l')
                .long("is-listener")
                .help("Act as the listencer during connection establishment")
                .action(ArgAction::SetTrue)
                .required(false),
        )
        .arg(
            Arg::new("duration")
                .long("duration")
                .help("Duration of experiment")
                .value_parser(clap::value_parser!(usize))
                .default_value("10")
                .required(false),
        )
        .arg(
            Arg::new("ratio")
                .long("ratio")
                .help("ratio of local")
                .value_parser(clap::value_parser!(usize))
                .default_value("50")
                .required(false),
        )
        .get_matches();

    let run_fix = matches.get_flag("fix");

    let smp = *matches.get_one::<usize>("smp").unwrap_or(&max_smp);
    let cid = *matches.get_one::<usize>("cid").unwrap();
    let ratio = *matches.get_one::<usize>("ratio").unwrap();
    let iam = matches.get_one::<String>("iam").unwrap();
    let peer = matches.get_one::<String>("peer").unwrap();
    let is_listener = matches.get_flag("listener");
    let duration = *matches.get_one::<usize>("duration").unwrap();

    // TODO(kmohr): can this create an invalid port number?
    let host_listener_port = (cid as u32) + 1561;

    let mut runtime = if run_fix {
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
        let dir = FsDir::new("/tmp", Open::ReadWrite).unwrap();
        s.add_blocking("", dir);
        let tcp = TcpFS::default();
        s.add_blocking("tcp", tcp);

        // NOTE: this assumes you have ppm files in $HOME/data/
        let shared_data_dir = FsDir::new(concat!(env!("HOME"), "/data/"), Open::ReadWrite).unwrap();
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

    // Setup Tcp Connection
    let (server_conn, client_conn) = if is_listener {
        let listener = TcpListener::bind(iam)?;
        let conn1 = listener.accept()?;
        let conn2 = listener.accept()?;
        (conn1.0, conn2.0)
    } else {
        let conn1 = TcpStream::connect(peer)?;
        let conn2 = TcpStream::connect(peer)?;
        (conn2, conn1)
    };

    let iam_ipaddr = u64::from(IpAddr::try_from(iam.as_str()).unwrap());
    let peer_ipaddr = u64::from(IpAddr::try_from(peer.as_str()).unwrap());

    log::info!(
        "Running {} on VM cid={} with {} core(s) for {}s. I am {} and peer is {}",
        if run_fix { "fix" } else { "arcade" },
        cid,
        smp,
        duration,
        iam,
        peer,
    );

    // XXX: this will break if usize is smaller than u64
    runtime.run(
        &[
            cid,
            host_listener_port as usize,
            iam_ipaddr as usize,
            peer_ipaddr as usize,
            is_listener as usize,
            duration,
            ratio,
        ],
        server_conn,
        client_conn,
        is_listener,
    );

    Ok(())
}
