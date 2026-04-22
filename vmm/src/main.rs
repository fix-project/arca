use std::path::PathBuf;

use clap::Parser;
use vmm::runtime::Runtime;

#[derive(Parser, Debug)]
struct Args {
    kernel: PathBuf,
    #[arg(short, long, env = "ARCA_SMP")]
    smp: Option<usize>,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();
    let smp = args
        .smp
        .or_else(|| std::thread::available_parallelism().ok().map(|x| x.get()))
        .unwrap_or(1);

    let bin = std::fs::read(args.kernel)?;
    let mut rt = Runtime::new(smp, 1 << 34, bin.into());
    rt.run(&[]);

    Ok(())
}
