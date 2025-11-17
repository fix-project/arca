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

use core::time::Duration;

use ::vfs::*;
use alloc::format;
use common::util::descriptors::Descriptors;
use kernel::{prelude::*, rt};
use ninep::Client;

mod dev;
mod vsock;

mod proc;
use crate::{
    dev::DevFS,
    proc::{Env, Namespace, Proc, ProcState, namespace::MountType},
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

const MACHINE: i32 = 1;
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
    let tcp_port = args[2];

    // let mut fd = Descriptors::new();

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

    // fd.set(0, stdin.into());
    // fd.set(1, stdout.into());
    // fd.set(2, stderr.into());

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
    let data = remote.attach(None, "", "data").await.unwrap();

    ns.mkdir("/n").await.unwrap();
    ns.mkdir("/n/host").await.unwrap();

    ns.attach(host, "/n/host", MountType::Replace, true)
        .await
        .unwrap();
    ns.attach(tcp, "/net/tcp", MountType::Replace, true)
        .await
        .unwrap();

    if MACHINE == 0 {
        // Spawn TCP server loop on a separate thread/core
        // rt::spawn(async move {
        log::info!("TCP server starting on separate thread");

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
        let id: usize = id.parse().unwrap();

        log::info!("Got TCP connection ID: {}", id);

        // Listen to incoming connections on the specified tcp_port
        let tcp_announcement = alloc::format!("announce 127.0.0.1:{}\n", tcp_port);
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

        // Continually listen for incoming connections
        loop {
            log::info!("Waiting for connections...");
            let mut listen_file = ns
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

            log::info!("New connection accepted: {}", id);

            // Get data file for the accepted connection
            let mut data_file = ns
                .walk(&data_path, Open::ReadWrite)
                .await
                .unwrap()
                .as_file()
                .unwrap();

            // Read data from the connection - handle arbitrarily large messages
            let mut all_data = Vec::new();
            let mut buffer = vec![0u8; 4096]; // Larger buffer for better performance

            loop {
                match data_file.read(&mut buffer).await {
                    Ok(bytes_read) => {
                        if bytes_read > 0 {
                            all_data.extend_from_slice(&buffer[..bytes_read]);
                        } else {
                            // bytes_read == 0 means the connection is closed
                            log::info!("Connection closed by client");
                            break;
                        }
                    }
                    Err(e) => {
                        log::error!("Error reading data: {:?}", e);
                        break;
                    }
                }
            }

            if !all_data.is_empty() {
                log::info!("Received complete message of {} bytes", all_data.len());
                // verify the message with a crc32 hash
                log::info!(
                    "CRC32 hash of received message: {:08x}",
                    crc32fast::hash(&all_data)
                );

                // TODO(kmohr) just assuming this is a continuation for now
                match postcard::from_bytes(&all_data).unwrap() {
                    Value::Function(k) => {
                        let result = k.apply(Word::new(0)).force();
                        log::info!("Continuation result: {:?}", result);
                    }
                    _ => {
                        log::error!("Received message is not a Function!");
                    }
                }
            }

            // Connection is automatically dropped when data_file goes out of scope
            // The TCP connection will be closed
            log::info!("Dropping connection {}", id);
        }
    } else {
        let x: Result<u64> = try {
            let f = common::elfloader::load_elf(THUMBNAILER).unwrap();
            let result = run(f, ns.clone()).await?;
            result
        };

        match x {
            Ok(result) => {
                log::info!("result: {result}");
            }
            Err(_) => {
                log::warn!("addition failed")
            }
        }
    }
}

async fn run(mut f: Function, ns: Namespace) -> Result<u64> {
    let mut counter: u64 = 0;

    loop {
        let result = f.force();

        let g = match result {
            Value::Function(g) => g,
            Value::Word(x) => return Ok(x.read().try_into().unwrap()),
            _ => {
                log::error!("proc returned something other than an effect or number!");
                return Err(ErrorKind::Unsupported.into());
            }
        };

        if g.is_arcane() {
            f = g;
            continue;
        }

        let Value::Tuple(mut data) = g.into_inner().read() else {
            unreachable!()
        };
        let t: Blob = data.take(0).try_into().unwrap();
        assert_eq!(&*t, b"Symbolic");
        let effect: Blob = data.take(1).try_into().unwrap();
        let args: Tuple = data.take(2).try_into().unwrap();
        let mut args: Vec<Value> = args.into_iter().collect();
        let Some(Value::Function(k)) = args.pop() else {
            return Err(ErrorKind::Other.into());
        };

        f = match (&*effect, &mut *args) {
            (b"add", [Value::Word(l), Value::Word(r)]) => k.apply(l.read() + r.read()),
            (b"incr", []) => {
                counter += 1;
                k.apply(counter)
            }
            (b"get", []) => {
                // Send "hello, world!" to localhost:11234 and then exit this process

                let send_tcp_msg = || async {
                    // Get a new TCP connection
                    let tcp_ctl_result = ns.walk("/net/tcp/clone", vfs::Open::ReadWrite).await;
                    let mut tcp_ctl = match tcp_ctl_result {
                        Ok(obj) => match obj.as_file() {
                            Ok(file) => file,
                            Err(_) => {
                                log::error!("Failed to get TCP control file");
                                return 1;
                            }
                        },
                        Err(e) => {
                            log::error!("Failed to walk to /net/tcp/clone: {:?}", e);
                            return 1;
                        }
                    };

                    // Read the connection ID
                    let mut id_buf = [0u8; 32];
                    let size = match tcp_ctl.read(&mut id_buf).await {
                        Ok(s) => s,
                        Err(e) => {
                            log::error!("Failed to read connection ID: {:?}", e);
                            return 1;
                        }
                    };
                    let conn_id = match core::str::from_utf8(&id_buf[..size]) {
                        Ok(s) => s.trim(),
                        Err(_) => {
                            log::error!("Invalid UTF-8 in connection ID");
                            return 1;
                        }
                    };

                    // Connect to localhost:11234
                    let connect_cmd = alloc::format!("connect 127.0.0.1:11212\n");
                    if let Err(e) = tcp_ctl.write(connect_cmd.as_bytes()).await {
                        log::error!("Failed to send connect command: {:?}", e);
                        return 1;
                    }

                    // Get the data file for this connection
                    let data_path = alloc::format!("/net/tcp/{}/data", conn_id);
                    let mut data_file = match ns.walk(&data_path, vfs::Open::ReadWrite).await {
                        Ok(obj) => match obj.as_file() {
                            Ok(file) => file,
                            Err(_) => {
                                log::error!("Failed to get data file");
                                return 1;
                            }
                        },
                        Err(e) => {
                            log::error!("Failed to walk to data path: {:?}", e);
                            return 1;
                        }
                    };

                    // Send continuation
                    let val = Value::Function(k);
                    let message = postcard::to_allocvec(&val).unwrap();

                    // log the message size in bytes and MB
                    let size_bytes = message.len();
                    let size_mb = size_bytes as f64 / (1024.0 * 1024.0);
                    log::info!(
                        "Sending message of size: {} bytes ({:.2} MB)",
                        size_bytes,
                        size_mb
                    );
                    // hash this so we can verify it on the other side
                    let hash = crc32fast::hash(&message);
                    log::info!("Message CRC32 hash: {:08x}", hash);

                    if let Err(e) = data_file.write(&message).await {
                        log::error!("Failed to send message: {:?}", e);
                        return 1;
                    }

                    log::info!("Sent serialized continuation to localhost:11234");
                    return 0;
                };

                let ret_code = send_tcp_msg().await;
                // we don't actually need to run k anymore
                return Ok(ret_code as u64);
            }
            _ => panic!("invalid effect: {effect:?}({args:?})"),
        };
    }
}
