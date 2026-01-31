#![feature(allocator_api)]
#![feature(thread_sleep_until)]
#![feature(future_join)]

use std::path::PathBuf;

use clap::Parser;
use vmm::runtime::Runtime;

#[derive(Parser, Debug)]
struct Args {
    kernel: PathBuf,
    #[arg(short, long)]
    smp: Option<usize>,
    #[arg(short, long, default_value = "3")]
    cid: usize,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();
    let smp = args
        .smp
        // .or_else(|| std::thread::available_parallelism().ok().map(|x| x.get()))
        .unwrap_or(1);
    let cid = args.cid;

    let bin = std::fs::read(args.kernel)?;
    let mut rt = Runtime::new(cid, smp, 1 << 34, bin.into());
    rt.run(&[]);

    Ok(())
}
