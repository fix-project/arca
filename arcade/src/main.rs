#![no_main]
#![no_std]
#![feature(try_blocks)]
#![feature(try_trait_v2)]
#![feature(iterator_try_collect)]
#![feature(box_patterns)]
#![feature(never_type)]
#![feature(ptr_metadata)]
#![feature(allocator_api)]
#![allow(dead_code)]
#![cfg_attr(feature = "testing-mode", allow(unreachable_code))]
#![cfg_attr(feature = "testing-mode", allow(unused))]

#[cfg(feature = "testing-mode")]
mod testing;

use ::vfs::*;
use alloc::format;
use common::util::descriptors::Descriptors;
use kernel::prelude::*;

mod dev;
mod vsock;

use async_lock::RwLock;

mod proc;
use crate::{
    dev::DevFS,
    proc::{Env, Namespace, Proc, ProcState, namespace::MountType},
    vsock::VSockFS,
};
use vfs::mem::MemDir;

extern crate alloc;

#[kmain]
async fn main(argv: &[usize]) {
    #[cfg(feature = "testing-mode")]
    {
        crate::testing::test_runner();
        return;
    }
    let &[ptr, len] = argv else {
        panic!("invalid arguments provided to arcade kernel");
    };

    let ptr: *mut u8 = BuddyAllocator.from_offset(ptr);
    let bin = unsafe { Box::from_raw_in(core::ptr::from_raw_parts_mut(ptr, len), BuddyAllocator) };
    log::info!("got bin of {len} bytes");

    let mut fd = Descriptors::new();

    let mut ns = Namespace::new(MemDir::default());

    let (stdin, stdout) = attach_dev(&mut ns).await;
    let stderr = stdout.dup().await.unwrap();

    fd.set(0, stdin.into());
    fd.set(1, stdout.into());
    fd.set(2, stderr.into());

    attach_vsock(&mut ns).await;

    let remote = connect_ninep(&mut ns).await;

    let host = remote.attach(None, "", "").await.unwrap();

    ns.mkdir("/n").await.unwrap();
    ns.mkdir("/n/host").await.unwrap();
    ns.attach(host, "/n/host", MountType::Replace, true)
        .await
        .unwrap();

    let tcp = remote.attach(None, "", "tcp").await.unwrap();
    ns.mkdir("/net/tcp").await.unwrap();
    ns.attach(tcp, "/net/tcp", MountType::Replace, true)
        .await
        .unwrap();

    let p = Proc::new(
        &bin,
        ProcState {
            ns: Arc::new(ns),
            env: Env::default().into(),
            fds: RwLock::new(fd).into(),
            cwd: PathBuf::from("/".to_owned()).into(),
        },
    )
    .unwrap();

    log::info!("starting");
    let exitcode = p.run([]).await;
    log::info!("exitcode: {exitcode}");
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

async fn connect_ninep(ns: &mut Namespace) -> ninep::Client {
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
    let remote = ninep::Client::new(data, |x| {
        kernel::rt::spawn(x);
    })
    .await
    .unwrap();
    remote
}
