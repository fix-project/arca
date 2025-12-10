#![no_main]
#![no_std]
#![feature(try_blocks)]
#![feature(try_trait_v2)]
#![feature(iterator_try_collect)]
#![feature(box_patterns)]
#![feature(never_type)]
#![allow(dead_code)]
#![cfg_attr(feature = "testing-mode", allow(unreachable_code))]
#![cfg_attr(feature = "testing-mode", allow(unused))]

#[cfg(feature = "testing-mode")]
mod testing;

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

extern crate alloc;

//const SERVER: &[u8] = include_bytes!("/tmp/hello");
//include_bytes!(env!("CARGO_BIN_FILE_MEMCACHED"));

#[kmain]
async fn main(_: &[usize]) {
    #[cfg(feature = "testing-mode")]
    {
        crate::testing::test_runner();
        return;
    }

    let mut fd = Descriptors::new();

    let mut ns = Namespace::new(MemDir::default());

    ns.mkdir("/net").await.unwrap();
    ns.mkdir("/net/tcp").await.unwrap();
    ns.mkdir("/net/vsock").await.unwrap();
    ns.mkdir("/dev").await.unwrap();
    ns.mkdir("/home").await.unwrap();
    ns.create("/home/example.txt", Create::default(), Open::ReadWrite).await.unwrap();
    //ns.create("/home/music.mp4", Create::default(), Open::ReadWrite).await.unwrap();

    ns.attach(DevFS::default(), "dev", MountType::Replace, true)
        .await
        .unwrap();

    let stdin = ns.walk("/dev/cons", Open::Read).await.unwrap();
    let stdout = ns.walk("/dev/cons", Open::Write).await.unwrap();
    let stderr = stdout.dup().await.unwrap();
    let home = ns.walk("/home", Open::ReadWrite).await.unwrap();
    //let mut music_file = ns.walk("/home/music.mp4", Open::ReadWrite).await.unwrap().as_file().unwrap();

    fd.set(0, stdin.into());
    fd.set(1, stdout.into());
    fd.set(2, stderr.into());

    fd.set(3, home.into());

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
    // let tcp = remote.attach(None, "", "tcp").await.unwrap();

    ns.mkdir("/n").await.unwrap();
    ns.mkdir("/n/host").await.unwrap();

    ns.attach(host, "/n/host", MountType::Replace, false).await.unwrap();
    let ffmpeg = ns.walk("/n/host/ffmpeg", Open::Read).await.unwrap();
    log::info!("walk ffmpeg");
    let mut file = ffmpeg.as_file().unwrap();
    let length = ns.walk("/n/host/ffmpeg.size", Open::Read).await.unwrap();
    log::info!("walk ffmpeg");
    let mut len = [0; 8];
    length.as_file().unwrap().read_exact(&mut len).await.unwrap();
    let len = usize::from_ne_bytes(len);
    log::info!("read size: {len}");
    let mut v = vec![0;len];
    file.read_exact(&mut v).await.unwrap();
    log::info!("{:?}", &v[0..4]);
    //let mut music = ns.walk("/n/host/fliparound.mp4", Open::Read).await.unwrap().as_file().unwrap();
    //let mut v_music = vec![0; 7383523];
    //music.read_exact(&mut v_music).await.unwrap();
    //music_file.write(&v_music);


    //     .await
    //     .unwrap();
    // ns.attach(tcp, "/net/tcp", MountType::Replace, true)
    //     .await
    //     .unwrap();

    let p = Proc::new(
        &v,
        ProcState {
            ns: Arc::new(ns),
            env: Env::default().into(),
            fds: RwLock::new(fd).into(),
            cwd: PathBuf::from("/n/host/".to_owned()).into(),
        },
    )
    .unwrap();
    kernel::profile::begin();

    rt::spawn(async {
        loop {
            log::info!("---");
            rt::profile();
            rt::reset_stats();
            rt::delay_for(Duration::from_secs(1)).await;
        }
    });

    //let exitcode = p.run(["adele.mp4"]).await;
    let exitcode = p.run(["ffmpeg\0", "-i\0", "weyes.flac\0", "-f\0", "wav\0", "weyes.wav\0"]).await;
    //let exitcode = p.run(["fmpeg\0", "-formats"]).await;
    //let exitcode = p.run(["ffmpeg\0", "-loglevel verbose", "-i ./example.txt -f mp3 music.mp3", ""]).await;
    //let exitcode = p.run(["ffmpeg\0", "-h\0"]).await;
    log::info!("exitcode: {exitcode}");
}
