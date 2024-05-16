use std::{fs::File, io::Write, path::PathBuf, process::Command};

use anyhow::Result;
use clap::Parser;
use tempfile::TempDir;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the kernel image
    #[arg(long)]
    kernel: PathBuf,

    /// Number of cores
    #[arg(long)]
    smp: Option<usize>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let bytes = include_bytes!(env!("CARGO_BIN_FILE_LOADER_loader"));
    let tempdir = TempDir::with_prefix("fix")?;
    let path = tempdir.path().join("kernel.elf");
    let mut file = File::create(&path)?;
    file.write_all(bytes)?;
    file.flush()?;

    let path = path.into_os_string().into_string().unwrap();
    let kernel = args.kernel.into_os_string().into_string().unwrap();
    let cpus = args
        .smp
        .or_else(|| std::thread::available_parallelism().ok().map(|x| x.get()))
        .unwrap_or(1);

    let mut qemu = Command::new("qemu-kvm");
    let qemu = qemu
        .args(["-machine", "microvm"])
        .args(["-monitor", "none"])
        .args(["-serial", "stdio"])
        .args(["-nographic"])
        .args(["-no-reboot"])
        .args(["-smp", &cpus.to_string()])
        .args(["-m", "4G"])
        .args(["-bios", "/usr/share/qemu/qboot.rom"])
        .args(["-kernel", &path])
        .args(["-device", &format!("loader,file={}", kernel)])
        .status()?;
    std::process::exit(qemu.code().unwrap_or(1));
}
