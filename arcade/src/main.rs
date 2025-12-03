#![no_main]
#![no_std]
#![feature(try_blocks)]
#![feature(try_trait_v2)]
#![feature(iterator_try_collect)]
#![feature(box_patterns)]
#![feature(never_type)]
#![feature(proc_macro_hygiene)]
#![allow(dead_code)]
#![cfg_attr(feature = "testing-mode", allow(unreachable_code))]
#![cfg_attr(feature = "testing-mode", allow(unused))]

use ::vfs::*;
use alloc::format;
use common::ipaddr::IpAddr;
use common::util::descriptors::Descriptors;
use core::str::FromStr;
use kernel::{kvmclock, prelude::*};
use ninep::Client;
// frame format isn't supported in no_std env
#[cfg(not(feature = "ablation"))]
use lz4_flex::block::decompress_size_prepended;

mod dev;
mod vsock;

mod proc;

mod tcpserver;
mod tcputil;

use crate::{
    dev::DevFS,
    proc::{Env, FileDescriptor, Namespace, Proc, ProcState, namespace::MountType},
    vsock::VSockFS,
};

use crate::tcpserver::TcpServer;
use vfs::mem::MemDir;

#[arca_module_test]
mod testing;

#[arca_module_test]
mod dummy_testing {
    #[arca_test]
    fn test_abc() {}
}

extern crate alloc;

const THUMBNAILER: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_THUMBNAIL_EXAMPLE"));

#[kmain]
async fn main(args: &[usize]) {
    #[cfg(feature = "testing-mode")]
    {
        crate::testing::__MODULE_TESTS.run();
        crate::dummy_testing::__MODULE_TESTS.run();
        return;
    }

    let cid = args[0] as u64;
    let host_listener_port = args[1];
    let tcp_port = IpAddr::from(args[2] as u64);
    let is_listener = args[3] != 0;

    let mut fd: Descriptors<FileDescriptor> = Descriptors::new();

    let mut ns = Namespace::new(MemDir::default());

    ns.mkdir("/net").await.unwrap();
    ns.mkdir("/net/tcp").await.unwrap();
    ns.mkdir("/net/vsock").await.unwrap();
    ns.mkdir("/dev").await.unwrap();
    ns.mkdir("/data").await.unwrap();

    ns.attach(DevFS::default(), "dev", MountType::Replace, true)
        .await
        .unwrap();

    let stdin = ns.walk("/dev/cons", Open::Read).await.unwrap();
    let stdout = ns.walk("/dev/cons", Open::Write).await.unwrap();
    let stderr = stdout.dup().await.unwrap();

    fd.set(0, stdin.into());
    fd.set(1, stdout.into());
    fd.set(2, stderr.into());

    ns.attach(VSockFS::new(cid), "/net/vsock", MountType::Replace, true)
        .await
        .unwrap();

    let mut ctl = ns
        .walk("/net/vsock/clone", Open::ReadWrite)
        .await
        .unwrap()
        .as_file()
        .unwrap();
    let mut id = [0; 32];
    let size = ctl.read(&mut id).await.unwrap();
    let id = core::str::from_utf8(&id[..size]).unwrap().trim();
    let id: usize = id.parse().unwrap();
    ctl.write(format!("connect 2:{}\n", host_listener_port).as_bytes())
        .await
        .unwrap();
    let data = ns
        .walk(format!("/net/vsock/{id}/data"), Open::ReadWrite)
        .await
        .unwrap()
        .as_file()
        .unwrap();
    let remote = Client::new(data, |x| {
        kernel::rt::spawn(x);
    })
    .await
    .unwrap();

    // Set up TCP connection
    let host = remote.attach(None, "", "").await.unwrap();
    let tcp = remote.attach(None, "", "tcp").await.unwrap();

    ns.mkdir("/n").await.unwrap();
    ns.mkdir("/n/host").await.unwrap();

    ns.attach(host, "/n/host", MountType::Replace, true)
        .await
        .unwrap();
    ns.attach(tcp, "/net/tcp", MountType::Replace, true)
        .await
        .unwrap();

    // TODO(kmohr): this runs on the assumption that the input file exists
    // locally in memory at the time the continuation is called.
    // Ideally, the continuation should be called as the file is read off the
    // network.
    let falls_ppm = include_bytes!(concat!(env!("HOME"), "/data/falls_1.ppm"));
    let memfs = MemDir::default();
    let mut falls_file = memfs
        .create("falls_1.ppm", Create::UserWrite, Open::ReadWrite)
        .await
        .unwrap()
        .as_file()
        .unwrap();
    falls_file.write(falls_ppm).await.unwrap();
    ns.attach(memfs, "/data", MountType::Replace, true)
        .await
        .unwrap();

    // Setup the tcp connection between the **two** machines
    let (server_conn, client_conn) = if is_listener {
        let listen_path = tcputil::listen_on(&ns, tcp_port)
            .await
            .expect("Failed to listen on port");
        let server_conn = tcputil::get_one_incoming_connection(&ns, listen_path.clone())
            .await
            .expect("Failed to get incoming connection");
        let client_conn = tcputil::get_one_incoming_connection(&ns, listen_path)
            .await
            .expect("Failed to get incoming connection");
        (server_conn, client_conn)
    } else {
        let client_conn = tcputil::connect_to(&ns, IpAddr::from_str("127.0.0.1:11212").unwrap())
            .await
            .expect("Failed to connect");
        let server_conn = tcputil::connect_to(&ns, IpAddr::from_str("127.0.0.1:11212").unwrap())
            .await
            .expect("Failed to connect");
        (server_conn, client_conn)
    };

    // Setup Handler
    if is_listener {
        // For now, have the listener only reponding to requests
        // let conn = Arc::new(SpinLock::new(conn));
        let shared_ns = Arc::new(ns);
        #[cfg(feature = "ablation")]
        {
            // Ablated case

            use crate::tcpserver::AblatedServer;
            let server = AblatedServer::new(server_conn, shared_ns);
            let tcpserver = TcpServer::new(server);

            let _ = tcpserver.run().await;
        }

        #[cfg(not(feature = "ablation"))]
        {
            // Nonablate case
            use crate::tcpserver::{ContinuationClient, ContinuationServer};
            let (sender, receiver) = channel::unbounded();
            let server = ContinuationServer::new(server_conn, sender);
            let tcpserver = TcpServer::new(server);
            let client = ContinuationClient::new(client_conn);

            kernel::rt::spawn(async move { tcpserver.run().await });

            loop {
                let data: Vec<u8> = receiver.recv().await.unwrap();
                let k_decompress_init = kvmclock::time_since_boot();
                let decompressed = decompress_size_prepended(&data).unwrap();
                let k_decompress_end = kvmclock::time_since_boot();
                match postcard::from_bytes(&decompressed).unwrap() {
                    Value::Function(k) => {
                        let p = Proc::from_function(
                            k,
                            ProcState {
                                ns: shared_ns.clone(),
                                env: Env::default().into(),
                                fds: RwLock::new(Descriptors::new()).into(),
                                cwd: PathBuf::from("/".to_owned()).into(),
                                host: Arc::new(tcp_port),
                            },
                            client.clone(),
                        )
                        .expect("Failed to create Proc from received Function");
                        let k_decode_end = kvmclock::time_since_boot();

                        let exitcode = p.run([]).await;

                        let run_k_end_time = kvmclock::time_since_boot();

                        log::info!(
                            //"TIMING:\nnetwork read: {} us\ndecompress: {} us\ndecode: {} us\nrun: {} us",
                            "TIMING:\ndecompress: {} us\ndecode: {} us\nrun: {} us",
                            //(get_k_end - get_k_init).as_micros(),
                            (k_decompress_end - k_decompress_init).as_micros(),
                            (k_decode_end - k_decompress_end).as_micros(),
                            (run_k_end_time - k_decode_end).as_micros()
                        );

                        log::info!("exitcode: {exitcode}");
                    }
                    _ => {
                        log::error!("Expected Function value in Continuation, got something else");
                    }
                }
            }
        }
    } else {
        let shared_ns = Arc::new(ns);
        let shared_port = Arc::new(tcp_port);

        let client = tcpserver::Client::new(client_conn);
        {
            let client = client.clone();
            kernel::rt::spawn(async move { client.run().await });
        }

        let env = Env::default();

        for _ in 0..100 {
            let start_time = kvmclock::time_since_boot();
            let p = Proc::new(
                THUMBNAILER,
                ProcState {
                    ns: shared_ns.clone(),
                    env: env.clone().into(),
                    fds: RwLock::new(Descriptors::new()).into(),
                    cwd: PathBuf::from("/".to_owned()).into(),
                    host: shared_port.clone(),
                },
                client.clone(),
            )
            .expect("Failed to create Proc from ELF");
            let exitcode = p.run([]).await;
            let end_time = kvmclock::time_since_boot();
            log::info!(
                "TIMING: begin k: {} us",
                (end_time - start_time).as_micros()
            );
            log::info!("exitcode: {exitcode}");
        }
    }
}
