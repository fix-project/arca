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
use kernel::{kvmclock, prelude::*};
use ninep::Client;
// frame format isn't supported in no_std env
use lz4_flex::block::decompress_size_prepended;

mod dev;
mod vsock;
mod proto;

mod proc;
use crate::{
    dev::DevFS,
    proc::{Env, FileDescriptor, Namespace, Proc, ProcState, namespace::MountType},
    vsock::VSockFS,
};
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
    let disable_continuation_sending = args[4] != 0;

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
    let falls_ppm = include_bytes!("/home/yuhan/data/falls_1.ppm");
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

    if is_listener {
        // Get id from /tcp/clone
        let mut tcp_ctl = ns
            .walk("/net/tcp/clone", Open::ReadWrite)
            .await
            .unwrap()
            .as_file()
            .unwrap();
        let mut id = [0; 32];
        let size = tcp_ctl.read(&mut id).await.unwrap();
        let id = core::str::from_utf8(&id[..size]).unwrap().trim();
        log::info!("TCP connection ID string: {}", id);
        let id: usize = id.parse().unwrap();

        log::info!("Got TCP connection ID: {}", id);

        // Listen to incoming connections on the specified tcp_port
        let tcp_port_str = tcp_port.to_string();
        log::info!("Listening on TCP port: {}", tcp_port_str);
        let tcp_announcement = alloc::format!("announce {}\n", tcp_port_str);
        log::info!("Announcing on: {}", tcp_announcement.trim());
        match tcp_ctl.write(tcp_announcement.as_bytes()).await {
            Ok(bytes_written) => {
                log::info!("Successfully wrote {} bytes to tcp control", bytes_written);
            }
            Err(e) => {
                log::error!("Failed to announce TCP listener: {:?}", e);
                return;
            }
        }

        let listen_path = format!("/net/tcp/{id}/listen");
        log::info!("Listen path: {}", listen_path);

        let shared_ns = Arc::new(ns);

        loop {
            // TODO(kmohr): handle multiple connections
            log::info!("Waiting for connections...");
            let mut listen_file = shared_ns
                .walk(&listen_path, Open::ReadWrite)
                .await
                .unwrap()
                .as_file()
                .unwrap();

            // Accept a new connection - this blocks until someone connects
            let mut id = [0; 32];
            let size = listen_file.read(&mut id).await.unwrap();
            let id = core::str::from_utf8(&id[..size]).unwrap().trim();
            let data_path = format!("/net/tcp/{id}/data");

            // TODO is this connection closed on Ctrl-C?
            log::info!("New connection accepted: {}", id);

            // TODO(kmohr): the time to actually read k off the network is way larger than
            // anything else. we should see what optimizations are possible here
            let get_k_init = kvmclock::time_since_boot();

            // Get data file for the accepted connection
            let mut data_file = shared_ns
                .walk(&data_path, Open::ReadWrite)
                .await
                .unwrap()
                .as_file()
                .unwrap();

            let (message, mut data_file) = proto::read_request(data_file).await.unwrap();
            let get_k_end = kvmclock::time_since_boot();

            match message {
                proto::Message::Continuation(proto::Continuation { continuation_size: _,  data }) => {
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
                        )
                        .expect("Failed to create Proc from received Function");
                        let k_decode_end = kvmclock::time_since_boot();

                        let exitcode = p.run([]).await;

                        let run_k_end_time = kvmclock::time_since_boot();

                        log::info!(
                            "TIMING:\nnetwork read: {} us\ndecompress: {} us\ndecode: {} us\nrun: {} us",
                            (get_k_end - get_k_init).as_micros(),
                            (k_decompress_end - k_decompress_init).as_micros(),
                            (k_decode_end - k_decompress_end).as_micros(),
                            (run_k_end_time - k_decode_end).as_micros()
                        );

                        log::info!("exitcode: {exitcode}");
                    },
                    _ => {
                        log::error!("Expected Function value in Continuation, got something else");
                    }
                }
            
                },
                proto::Message::FileRequest(proto::FileRequest{file_path}) => {
                    log::info!("Received File Request for path {}", file_path);
                    // send back the requested file data
                    let mut file = shared_ns
                        .walk(&file_path, Open::Read)
                        .await
                        .unwrap()
                        .as_file()
                        .unwrap();
                    
                    // TODO(kmohr) let's just encode the file size instead of reading in chunks like this
                    let mut file_data = Vec::new();
                    let mut buffer = [0u8; 4096];
                    loop {
                        match file.read(&mut buffer).await {
                            Ok(0) => break, // EOF
                            Ok(n) => file_data.extend_from_slice(&buffer[..n]),
                            Err(e) => {
                                log::error!("Failed to read file: {:?}", e);
                                break;
                            }
                        }
                    }
                    let file_size = file_data.len() as u32;

                    // send back b'F' + file size as u32 + file data
                    let mut response = alloc::vec![b'F'];
                    response.extend_from_slice(&file_size.to_be_bytes());
                    response.extend_from_slice(&file_data);
                    data_file.write(&response).await.unwrap();
                    log::info!("msg size: {}", response.len());
                    log::info!("Sent file response for {}", file_path);
                },
                _ => {
                    log::error!("Expected Continuation or FileRequest message, got something else");
                } 
            }
        }
    } else {
        let shared_ns = Arc::new(ns);
        let shared_port = Arc::new(tcp_port);

        let env = Env::default();
        env.set(
            "CONTINUATION_SENDING_ENABLED",
            if disable_continuation_sending { "0" } else { "1" },
        );

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
