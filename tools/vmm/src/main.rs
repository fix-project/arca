use std::{
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

use clap::Parser;
use vmm::runtime::Runtime;

#[derive(Parser, Debug)]
struct Args {
    kernel: PathBuf,
    #[arg(short, long, env = "ARCA_SMP")]
    smp: Option<usize>,
    #[arg(long, default_value_t = 16384)]
    memory: usize,
    argv: Vec<String>,
}

fn main() -> anyhow::Result<ExitCode> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();
    let smp = args
        .smp
        .or_else(|| std::thread::available_parallelism().ok().map(|x| x.get()))
        .unwrap_or(1);

    let bin = std::fs::read(args.kernel.clone())?;
    let ram = args
        .memory
        .checked_mul(1 << 20)
        .filter(|size| *size >= 16 << 20 && size.is_power_of_two())
        .ok_or_else(|| anyhow::anyhow!("memory must be a power of two, at least 16 MiB"))?;
    let mut rt = Runtime::new(smp, ram, bin.into());
    let interrupt = rt.interrupt_handle();
    let mut last_interrupt: Option<Instant> = None;
    ctrlc::set_handler(move || {
        let now = Instant::now();
        if last_interrupt
            .is_some_and(|previous| now.duration_since(previous) < Duration::from_secs(1))
        {
            std::process::exit(130);
        }
        let _ = interrupt.write(1);
        last_interrupt = Some(now);
    })?;
    let mut argv = args.argv;
    argv.insert(0, args.kernel.into_os_string().into_string().unwrap());
    Ok(ExitCode::from(rt.run(argv)))
}
