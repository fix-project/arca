#![no_main]
#![no_std]
#![feature(try_blocks)]
#![feature(try_trait_v2)]
#![feature(iterator_try_collect)]
#![feature(box_patterns)]
#![feature(never_type)]
#![feature(ptr_metadata)]
#![feature(allocator_api)]
#![feature(proc_macro_hygiene)]
#![allow(dead_code)]
#![cfg_attr(feature = "testing-mode", allow(unreachable_code))]
#![cfg_attr(feature = "testing-mode", allow(unused))]

#[cfg(feature = "testing-mode")]
mod testing;
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
// frame format isn't supported in no_std env
use lz4_flex::block::decompress_size_prepended;

mod dev;
mod vsock;

use async_lock::RwLock;
mod input_gen;
mod proc;

mod fileutil;
mod record;
mod tcpserver;
mod tcputil;

use crate::{
    dev::DevFS,
    fileutil::buffer_to_table,
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
    // let &[ptr, len] = argv else {
    //     panic!("invalid arguments provided to arcade kernel");
    // };

    // let ptr: *mut u8 = BuddyAllocator.from_offset(ptr);
    // let bin = unsafe { Box::from_raw_in(core::ptr::from_raw_parts_mut(ptr, len), BuddyAllocator) };
    // log::info!("got bin of {len} bytes");

    // let cid = args[0] as u64;
    let host_listener_port = args[1];
    let iam_ipaddr = IpAddr::from(args[2] as u64);
    let peer_ipaddr = IpAddr::from(args[3] as u64);
    let is_listener = args[4] != 0;
    let duration = Duration::from_secs(args[5] as u64);
    let ratio = args[6] as f64 / 100 as f64;

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

    let (stdin, stdout) = attach_dev(&mut ns).await;
    let stderr = stdout.dup().await.unwrap();

    fd.set(0, stdin.into());
    fd.set(1, stdout.into());
    fd.set(2, stderr.into());

    attach_vsock(&mut ns).await;

    let remote = connect_ninep(&mut ns, host_listener_port).await;

    // let host = remote.attach(None, "", "").await.unwrap();

    // ns.mkdir("/n").await.unwrap();
    // ns.mkdir("/n/host").await.unwrap();
    // ns.attach(host, "/n/host", MountType::Replace, true).await.unwrap();

    // let tcp = remote.attach(None, "", "tcp").await.unwrap();
    // ns.mkdir("/net/tcp").await.unwrap();
    // ns.attach(tcp, "/net/tcp", MountType::Replace, true)
    //     .await
    //     .unwrap();

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
        (String::from("falls_1.ppm"), 2332861),
        (String::from("Sun.ppm"), 12814240),
    ]));

    let mut img_names_to_contents: BTreeMap<String, Table> = BTreeMap::default();

    for (image_name, image_size) in img_names_to_sizes.iter() {
        let mut img_bytes = vec![0u8; *image_size];
        let mut image_file = remote_data
            .walk(image_name, Open::Read)
            .await
            .unwrap()
            .as_file()
            .unwrap();
        image_file.read(&mut img_bytes).await.unwrap();

        let table = buffer_to_table(&img_bytes);
        img_names_to_contents.insert(image_name.clone(), table);
    }

    let img_names_to_contents = Arc::new(img_names_to_contents);
    let shared_ns = Arc::new(ns);

    // Setup Servers
    #[cfg(feature = "ablation")]
    let (tcpserver_handle, continuation_receiver) = {
        use crate::tcpserver::AblatedServer;
        let server = AblatedServer::new(img_names_to_sizes.clone(), img_names_to_contents.clone());
        let tcpserver = TcpServer::new(server);
        let handle = kernel::rt::spawn(async move { tcpserver.run().await });

        let (sender, receiver) = channel::unbounded::<Option<Vec<u8>>>();
        sender
            .send_blocking(None)
            .expect("Failed to close sender side");
        (handle, receiver)
    };

    #[cfg(not(feature = "ablation"))]
    let (_tcpserver_handle, continuation_receiver) = {
        use crate::tcpserver::ContinuationServer;
        let (sender, receiver) = channel::unbounded();
        let server = ContinuationServer::new(sender);
        let tcpserver = TcpServer::new(server);
        let handle = kernel::rt::spawn(async move { tcpserver.run().await });
        (handle, receiver)
    };

    // Setup Clients
    let (client_tx, client_relay, client_rx) = tcpserver::make_client();
    let _client_relay_handle = kernel::rt::spawn(async move { client_relay.run().await });
    let _client_rx_handle = kernel::rt::spawn(async move { client_rx.run().await });

    let smp = kernel::ncores();
    let worker_thread_num = smp - 4;
    let total_count = Arc::new(AtomicUsize::new(0));
    let total_time = Arc::new(AtomicU64::new(0));
    let mut worker_threads = vec![];
    {
        let go = Arc::new(AtomicBool::new(false));
        let ready_count = Arc::new(AtomicUsize::new(0));

        for _ in 0..worker_thread_num {
            let go = go.clone();
            let total_count = total_count.clone();
            let total_time = total_time.clone();
            let ready_count = ready_count.clone();
            let shared_ns = shared_ns.clone();
            let img_names_to_sizes = img_names_to_sizes.clone();
            let img_names_to_contents = img_names_to_contents.clone();
            let client_tx = client_tx.clone();
            let continuation_receiver = continuation_receiver.clone();
            let mut host_gen = UnboundedInputHostGenerator::new(
                iam_ipaddr.to_string(),
                peer_ipaddr.to_string(),
                // 0.99,
                ratio,
            );

            worker_threads.push(kernel::rt::spawn(async move {
                let mut handle_one = async || -> Option<Record> {
                    if is_listener {
                      if let Ok(Some(continuation)) = continuation_receiver.recv().await
                      {
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
                                      img_names_to_contents.clone(),
                                  )
                                  .expect("Failed to create Proc from received Function");
                                  let k_decode_end = kvmclock::time_since_boot();

                                  let (exitcode, record) = p.run([]).await;

                                  log::debug!("exitcode: {exitcode}");
                                  let record = match record {
                                      Record::LocalRecord(local_record) => local_record,
                                      Record::RemoteDataRecord(_)
                                      | Record::MigratedRecord(_)
                                      | Record::RemoteInvocationRecord(_) => {
                                          panic!("Unexpected record type")
                                      }
                                  };

                                  let record: Record = RemoteInvocationRecord::new(
                                      k_decompress_end - k_decompress_init,
                                      k_decode_end - k_decompress_end,
                                      record,
                                  )
                                  .into();
                                  

                                  Some(record)
                              }
                              _ => {
                                  panic!(
                                      "Expected Function value in Continuation, got something else"
                                  );
                              }
                          }
                      } else { None }
                    } else {
                        let loading_elf_start = kvmclock::time_since_boot();
                        let thumbnailer_function = common::elfloader::load_elf(THUMBNAILER)
                            .expect("Failed to load ELF as Function");
                        let loading_elf_end = kvmclock::time_since_boot();

                        let image_hostname = host_gen.next().unwrap();
                        let image_filename = "Sun.ppm";
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
                            img_names_to_contents.clone(),
                        )
                        .expect("Failed to create Proc from ELF");
                        let (exitcode, mut record) = p.run([]).await;
                        log::debug!("exitcode: {exitcode}");
                        match &mut record {
                            Record::LocalRecord(local_record) => {
                                local_record.loading_elf = loading_elf_end - loading_elf_start;
                            }
                            Record::RemoteDataRecord(remote_data_record) => {
                                remote_data_record.loading_elf =
                                    loading_elf_end - loading_elf_start;
                            }
                            Record::MigratedRecord(migrated_record) => {
                                migrated_record.loading_elf = loading_elf_end - loading_elf_start;
                            }
                            Record::RemoteInvocationRecord(_) => panic!("Unexpected record type"),
                        }
                        Some(record)
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
                    if runtime >= duration {
                        break runtime;
                    }

                    let record = handle_one().await;
                    match record {
                        Some(record) => { accumulator.accumulate(record); },
                        None => { break runtime; },
                    }
                    count += 1;
                };
                total_count.fetch_add(count, Ordering::SeqCst);
                total_time.fetch_add(runtime.as_nanos() as u64, Ordering::SeqCst);
                accumulator
            }));
        }

        while ready_count.load(Ordering::Acquire) < worker_thread_num {
            kernel::rt::yield_now().await;
        }
        kernel::rt::delay_for(Duration::from_millis(1000)).await;
        go.store(true, Ordering::Release);
    }

    let mut accumulator = Accumulator::default();

    #[cfg(feature = "ablation")]
    {
       if !is_listener {
          log::debug!("Collecting worker thread metrics");
          for j in worker_threads {
              accumulator += j.await;
          }
          let total_time = total_time.load(Ordering::SeqCst);
          let total_count = total_count.load(Ordering::SeqCst) - accumulator.migrated_count;

          let freq = (total_count as f64 / (total_time as f64 / 1e9)) * worker_thread_num as f64;
          let freq_per_core = freq / worker_thread_num as f64;
          // let time_per_core = Duration::from_secs_f64(1. / freq) * worker_thread_num as u32;
          
          log::info!("{freq:10.2} Hz (per core: {freq_per_core:10.2} Hz");
          log::info!("{}", accumulator);
        }


        if is_listener {
          kernel::rt::delay_for(duration + Duration::from_secs(5)).await;
          //log::info!("Listener joining on tcpserver");
          // Close client_tx after main jobs are done
          //let _ = client_tx.close().await;
          //log::info!("Joining client relay");
          //let _ = client_relay_handle.await;
          //log::info!("Joining client rx");
          //let _ = client_rx_handle.await;
          //      log::info!("Joining tcpserver");
          //let _ = tcpserver_handle.await;
        }
    }

    #[cfg(not(feature = "ablation"))]
    {
          log::debug!("Collecting worker thread metrics");
          for j in worker_threads {
              accumulator += j.await;
          }
          let total_time = total_time.load(Ordering::SeqCst);
          let total_count = total_count.load(Ordering::SeqCst) - accumulator.migrated_count;

          let freq = (total_count as f64 / (total_time as f64 / 1e9)) * worker_thread_num as f64;
          let freq_per_core = freq / worker_thread_num as f64;
          // let time_per_core = Duration::from_secs_f64(1. / freq) * worker_thread_num as u32;
          
          log::info!("{freq:10.2} Hz (per core: {freq_per_core:10.2} Hz");
          log::info!("{}", accumulator);
    }

    // let p = Proc::new(
    //     &bin,
    //     ProcState {
    //         ns: Arc::new(ns),
    //         env: Env::default().into(),
    //         fds: RwLock::new(fd).into(),
    //         cwd: PathBuf::from("/".to_owned()).into(),
    //     },
    // )
    // .unwrap();

    // log::info!("starting");
    // let exitcode = p.run([]).await;
    // log::info!("exitcode: {exitcode}");
}

async fn attach_dev(ns: &mut Namespace) -> (Object, Object) {
    ns.mkdir("/dev").await.unwrap();
    ns.attach(DevFS::default(), "/dev", MountType::Replace, true)
        .await
        .unwrap();
    (
        ns.walk("/dev/cons", Open::Read).await.unwrap(),
        ns.walk("/dev/cons", Open::Write).await.unwrap(),
    )
}

async fn attach_vsock(ns: &mut Namespace) {
    ns.mkdir("/net").await.unwrap();
    ns.mkdir("/net/vsock").await.unwrap();
    ns.attach(VSockFS::default(), "/net/vsock", MountType::Replace, true)
        .await
        .unwrap();
}

async fn connect_ninep(ns: &mut Namespace, host_listener_port: usize) -> ninep::Client {
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
    let remote = ninep::Client::new(data, |x| {
        kernel::rt::spawn(x);
    })
    .await
    .unwrap();
    remote

}
