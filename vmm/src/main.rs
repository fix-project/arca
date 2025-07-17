#![feature(allocator_api)]
#![feature(thread_sleep_until)]
#![feature(future_join)]

use std::{
    io::{Read, Write as _},
    num::NonZero,
};

use vmm::runtime::Runtime;

use vsock::VsockStream;

const KERNEL_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_KERNEL_kernel"));

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let smp = std::thread::available_parallelism()
        .unwrap_or(NonZero::new(1).unwrap())
        .get();
    let mut runtime = Runtime::new(smp, 1 << 30, KERNEL_ELF.into());

    let cid = runtime.cid();
    std::thread::spawn(move || {
        let mut stream = VsockStream::connect_with_cid_port(cid as u32, 80).unwrap();
        stream.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();
        let mut buf = vec![];
        log::warn!("reading");
        stream.read_to_end(&mut buf).unwrap();
        log::warn!("done");
        log::warn!("response: {buf:?}");
    });

    runtime.run(&[]);

    Ok(())
}
