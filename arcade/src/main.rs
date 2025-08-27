#![no_main]
#![no_std]
#![feature(try_blocks)]
#![feature(try_trait_v2)]
#![feature(iterator_try_collect)]
#![feature(box_patterns)]
#![feature(never_type)]
#![allow(dead_code)]

use ::vfs::*;
use alloc::format;
use common::util::descriptors::Descriptors;
use kernel::{prelude::*, profile};
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

extern crate alloc;

const SERVER: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_server"));

#[kmain]
async fn main(_: &[usize]) {
    let mut fd = Descriptors::new();

    let mut ns = Namespace::new(MemDir::default());

    ns.mkdir("/net").await.unwrap();
    ns.mkdir("/net/tcp").await.unwrap();
    ns.mkdir("/net/vsock").await.unwrap();
    ns.mkdir("/dev").await.unwrap();

    ns.attach(DevFS::default(), "dev", MountType::Replace, true)
        .await
        .unwrap();

    let stdin = ns.walk("/dev/cons", Open::Read).await.unwrap();
    let stdout = ns.walk("/dev/cons", Open::Write).await.unwrap();
    let stderr = stdout.dup().await.unwrap();

    fd.set(0, stdin.into());
    fd.set(1, stdout.into());
    fd.set(2, stderr.into());

    ns.attach(VSockFS::default(), "/net/vsock", MountType::Replace, true)
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
    ctl.write(b"connect 2:1564\n").await.unwrap();
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

    let p = Proc::new(
        SERVER,
        ProcState {
            ns: Arc::new(ns),
            env: Env::default().into(),
            fds: RwLock::new(fd).into(),
            cwd: PathBuf::from("/".to_owned()).into(),
        },
    )
    .unwrap();
    profile::begin();
    let exitcode = p.run([]).await;
    log::info!("exitcode: {exitcode}");
}
