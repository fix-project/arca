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

use core::{
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    time::Duration,
};

use ::vfs::*;
use alloc::collections::BTreeMap;
use alloc::format;
use common::ipaddr::IpAddr;
use common::util::descriptors::Descriptors;
use kernel::{kvmclock, prelude::*};
use ninep::Client;
// frame format isn't supported in no_std env
use lz4_flex::block::decompress_size_prepended;

mod dev;
mod vsock;

mod input_gen;
mod proc;

mod record;
mod tcpserver;
mod tcputil;

use crate::{
    dev::DevFS,
    proc::{Env, FileDescriptor, Namespace, Proc, ProcState, namespace::MountType},
    record::{Accumulator, Record, RemoteInvocationRecord},
    vsock::VSockFS,
};

use crate::input_gen::UnboundedInputHostGenerator;
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
    let iam_ipaddr = IpAddr::from(args[2] as u64);
    let peer_ipaddr = IpAddr::from(args[3] as u64);
    let is_listener = args[4] != 0;
    let duration = Duration::from_secs(args[5] as u64);

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

    // copy listed files from /data into the in-memory fs
    let remote_data = remote.attach(None, "", "data").await.unwrap();

    // TODO: read from the directory instead of hardcoding filenames and sizes
    let img_names_to_sizes = Arc::new(BTreeMap::from([
        ("falls_1.ppm", 2332861),
        ("Sun.ppm", 12814240),
    ]));

    let local_data = MemDir::default();
    for (image_name, image_size) in img_names_to_sizes.iter() {
        let mut img_bytes = vec![0u8; *image_size];
        let mut image_file = remote_data
            .walk(image_name, Open::Read)
            .await
            .unwrap()
            .as_file()
            .unwrap();
        image_file.read(&mut img_bytes).await.unwrap();

        let mut mem_image_file = local_data
            .create(image_name, Create::UserWrite, Open::ReadWrite)
            .await
            .unwrap()
            .as_file()
            .unwrap();
        mem_image_file.write(&img_bytes).await.unwrap();
    }
    ns.attach(local_data, "/data", MountType::Replace, true)
        .await
        .unwrap();

    // Setup the tcp connection between the **two** machines
    let (server_conn, client_conn) = if is_listener {
        let listen_path = tcputil::listen_on(&ns, iam_ipaddr)
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
        let client_conn = tcputil::connect_to(&ns, peer_ipaddr)
            .await
            .expect("Failed to connect");
        let server_conn = tcputil::connect_to(&ns, peer_ipaddr)
            .await
            .expect("Failed to connect");
        (server_conn, client_conn)
    };

    let shared_ns = Arc::new(ns);

    // Setup Servers
    #[cfg(feature = "ablation")]
    let (tcpserver_handle, continuation_receiver) = {
        use crate::tcpserver::AblatedServer;
        let (read_half, write_half) = server_conn;
        let server = AblatedServer::new(read_half, write_half, shared_ns.clone());
        let tcpserver = TcpServer::new(server);
        let handle = kernel::rt::spawn(async move { tcpserver.run().await });

        let (sender, receiver) = channel::unbounded::<Option<Vec<u8>>>();
        sender
            .send(None)
            .await
            .expect("Failed to close sender side");
        (handle, receiver)
    };

    #[cfg(not(feature = "ablation"))]
    let (tcpserver_handle, continuation_receiver) = {
        use crate::tcpserver::ContinuationServer;
        let (sender, receiver) = channel::unbounded();
        let (read_half, write_half) = server_conn;
        let server = ContinuationServer::new(read_half, write_half, sender);
        let tcpserver = TcpServer::new(server);
        let handle = kernel::rt::spawn(async move { tcpserver.run().await });
        (handle, receiver)
    };

    // Setup Clients
    let (read_half, write_half) = client_conn;
    let (client_tx, client_relay, client_rx) = tcpserver::make_client(read_half, write_half);
    let client_relay_handle = kernel::rt::spawn(async move { client_relay.run().await });
    let client_rx_handle = kernel::rt::spawn(async move { client_rx.run().await });

    let smp = kernel::ncores();
    let total_count = Arc::new(AtomicUsize::new(0));
    let total_time = Arc::new(AtomicU64::new(0));
    let mut worker_threads = vec![];
    {
        let go = Arc::new(AtomicBool::new(false));
        let ready_count = Arc::new(AtomicUsize::new(0));

        for _ in 0..smp {
            let go = go.clone();
            let total_count = total_count.clone();
            let total_time = total_time.clone();
            let ready_count = ready_count.clone();
            let shared_ns = shared_ns.clone();
            let img_names_to_sizes = img_names_to_sizes.clone();
            let client_tx = client_tx.clone();
            let continuation_receiver = continuation_receiver.clone();
            let mut host_gen = UnboundedInputHostGenerator::new(
                iam_ipaddr.to_string(),
                peer_ipaddr.to_string(),
                // 0.99,
                0.0,
            );

            worker_threads.push(kernel::rt::spawn(async move {
                let mut handle_one = async || -> (Record, Duration) {
                    if let Some(Ok(Some(continuation))) = continuation_receiver.try_recv() {
                        let k_decompress_init = kvmclock::time_since_boot();
                        let decompressed = decompress_size_prepended(&continuation).unwrap();
                        let k_decompress_end = kvmclock::time_since_boot();
                        match postcard::from_bytes(&decompressed).unwrap() {
                            Value::Function(k) => {
                                let p = Proc::from_remote_function(
                                    k,
                                    ProcState {
                                        ns: shared_ns.clone(),
                                        env: Env::default().into(),
                                        fds: RwLock::new(Descriptors::new()).into(),
                                        cwd: PathBuf::from("/".to_owned()).into(),
                                        host: Arc::new(iam_ipaddr),
                                    },
                                )
                                .expect("Failed to create Proc from received Function");
                                let k_decode_end = kvmclock::time_since_boot();

                                let (exitcode, _) = p.run([]).await;

                                let run_k_end_time = kvmclock::time_since_boot();

                                log::debug!("exitcode: {exitcode}");

                                (
                                    RemoteInvocationRecord {
                                        decompression: k_decompress_end - k_decompress_init,
                                        deserialization: k_decode_end - k_decompress_end,
                                        execution: run_k_end_time - k_decode_end,
                                    }
                                    .into(),
                                    Duration::default(),
                                )
                            }
                            _ => {
                                panic!(
                                    "Expected Function value in Continuation, got something else"
                                );
                            }
                        }
                    } else {
                        let start_time = kvmclock::time_since_boot();

                        let thumbnailer_function = common::elfloader::load_elf(THUMBNAILER)
                            .expect("Failed to load ELF as Function");

                        let image_hostname = host_gen.next().unwrap();
                        let image_filename = "falls_1.ppm";
                        let image_size = img_names_to_sizes[image_filename];

                        let filepath = arca::Value::Blob(arca::Blob::from(
                            format!("{}/data/{}", image_hostname, image_filename).as_bytes(),
                        ));
                        let image_filesize = arca::Value::Word(arca::Word::new(image_size as u64));

                        let p = Proc::from_function(
                            thumbnailer_function.apply(filepath).apply(image_filesize),
                            ProcState {
                                ns: shared_ns.clone(),
                                env: Env::default().into(),
                                fds: RwLock::new(Descriptors::new()).into(),
                                cwd: PathBuf::from("/".to_owned()).into(),
                                host: iam_ipaddr.into(),
                            },
                            client_tx.clone(),
                        )
                        .expect("Failed to create Proc from ELF");
                        let (exitcode, record) = p.run([]).await;
                        let end_time = kvmclock::time_since_boot();
                        log::debug!("exitcode: {exitcode}");
                        (record, end_time - start_time)
                    }
                };

                let mut count = 0;
                ready_count.fetch_add(1, Ordering::Release);

                while !go.load(Ordering::Acquire) {
                    handle_one().await;
                    kernel::rt::yield_now().await;
                }

                let exp_start = kvmclock::time_since_boot();
                let mut accumulator = Accumulator::default();

                let runtime = loop {
                    let runtime = kvmclock::time_since_boot() - exp_start;
                    if &runtime >= &duration {
                        break runtime;
                    }

                    let (record, d) = handle_one().await;
                    accumulator.accumulate(record, d);
                    count += 1;
                };
                total_count.fetch_add(count, Ordering::SeqCst);
                total_time.fetch_add(runtime.as_nanos() as u64, Ordering::SeqCst);
                accumulator
            }));
        }

        while ready_count.load(Ordering::Acquire) < smp {
            kernel::rt::yield_now().await;
        }
        kernel::rt::delay_for(Duration::from_millis(1000)).await;
        go.store(true, Ordering::Release);
    }

    let mut accumulator = Accumulator::default();

    log::debug!("Collecting worker thread metrics");
    for j in worker_threads {
        accumulator += j.await;
    }
    let total_time = total_time.load(Ordering::SeqCst);
    let total_count = total_count.load(Ordering::SeqCst);

    let freq = (total_count as f64 / (total_time as f64 / 1e9)) * smp as f64;
    let freq_per_core = freq / smp as f64;
    let time_per_core = Duration::from_secs_f64(1. / freq) * smp as u32;

    // Close client_tx after main jobs are done
    let _ = client_tx.close().await;
    log::debug!("Joining client relay");
    let _ = client_relay_handle.await;
    log::debug!("Joining client rx");
    let _ = client_rx_handle.await;
    log::debug!("Joining tcpserver");
    let _ = tcpserver_handle.await;

    log::info!("{freq:10.2} Hz (per core: {freq_per_core:10.2} Hz, {time_per_core:?} per iter)");
    log::info!("{}", accumulator);
}
