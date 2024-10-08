use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tempfile::NamedTempFile;
use xshell::{cmd, Shell};

static CARGO: LazyLock<String> =
    LazyLock::new(|| std::env::var("CARGO").unwrap_or("cargo".to_string()));

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
    #[command(subcommand)]
    command: SubCommand,
}

#[derive(Subcommand)]
enum SubCommand {
    Build {
        /// Build in release mode
        #[clap(short, long, default_value_t = false)]
        release: bool,
    },
    Run {
        /// Build and run in release mode
        #[clap(short, long, default_value_t = false)]
        release: bool,
        /// The number of vCPUs to use (defaults to the number of hardware CPUs).
        #[clap(long)]
        smp: Option<usize>,
        /// Wait for a GDB connection
        #[clap(long, default_value_t = false)]
        gdb: bool,
        /// Print extra debugging info from QEMU
        #[clap(long, default_value_t = false)]
        debug: bool,
        /// Kernel command-line arguments
        #[clap(last = true)]
        args: Vec<String>,
    },
    Test {
        /// Build and run in release mode
        #[clap(short, long, default_value_t = false)]
        release: bool,
        /// The number of vCPUs to use (defaults to the number of hardware CPUs).
        #[clap(long)]
        smp: Option<usize>,
        /// Wait for a GDB connection
        #[clap(long, default_value_t = false)]
        gdb: bool,
        /// Kernel command-line arguments
        args: Vec<String>,
    },
}

fn build(sh: &Shell, package: &str, extra_flags: &[&str], target: &str) -> Result<Vec<PathBuf>> {
    let cargo: &str = &CARGO;
    let info = cmd!(
        sh,
        "{cargo} build -p {package} --target {target} --message-format=json-render-diagnostics"
    )
    .args(extra_flags)
    .read()?;
    let mut executables = vec![];
    for line in info.lines() {
        let msg = json::parse(line)?;
        if msg["reason"] == "compiler-artifact" && !msg["executable"].is_null() {
            executables.push(PathBuf::from(&msg["executable"].as_str().unwrap()));
        }
    }
    Ok(executables)
}

struct Config<'a> {
    kernel: &'a Path,
    smp: usize,
    debug: bool,
    gdb: bool,
    args: &'a [String],
    test: bool,
}

fn run(sh: &Shell, config: &Config) -> Result<()> {
    let &Config {
        kernel,
        smp,
        debug,
        gdb,
        args,
        test,
    } = config;
    let kernel = kernel.display().to_string();
    let bin = NamedTempFile::with_prefix("kernel-")?;
    let name = bin.path();
    cmd!(sh, "objcopy -Oelf32-i386 {kernel} {name}").run()?;
    let smp = smp.to_string();

    let qemu = cmd!(sh, "qemu-system-x86_64 -cpu host,+invtsc,+vmware-cpuid-freq -machine microvm -enable-kvm -monitor none -serial none -debugcon stdio -nographic -no-reboot -smp {smp} -m 4G -bios /usr/share/qemu/qboot.rom -kernel {name}");

    let qemu = if test {
        qemu.args(["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"])
    } else {
        qemu
    };

    let qemu = if debug {
        qemu.args(["-d", "guest_errors"])
    } else {
        qemu
    };

    let qemu = if gdb {
        println!("starting gdb server on port 1234 and awaiting connection");
        qemu.args(["-s", "-S"])
    } else {
        qemu
    };

    let qemu = if !args.is_empty() {
        qemu.args(["-append", &args.join(" ")])
    } else {
        qemu
    };

    qemu.run()?;
    bin.close()?;
    Ok(())
}

fn main() -> Result<()> {
    let sh = Shell::new()?;
    let args = Args::parse();

    match args.command {
        SubCommand::Build { release } => {
            let mut args = vec![];
            if release {
                args.push("--release");
            }
            build(&sh, "kernel", &args, "x86_64-unknown-none")?;
            Ok(())
        }
        SubCommand::Run {
            release,
            smp,
            debug,
            gdb,
            args,
        } => {
            let mut flags = vec![];
            if release {
                flags.push("--release");
            }
            let kernel = &build(&sh, "kernel", &flags, "x86_64-unknown-none")?[0];
            let smp = smp
                .or_else(|| std::thread::available_parallelism().ok().map(|x| x.get()))
                .unwrap_or(1);
            run(
                &sh,
                &Config {
                    kernel,
                    smp,
                    debug,
                    gdb,
                    args: &args,
                    test: false,
                },
            )
        }
        SubCommand::Test {
            release,
            smp,
            gdb,
            args,
        } => {
            let mut flags = vec![];
            if release {
                flags.push("--release");
            }
            flags.push("--tests");
            let tests = &build(&sh, "kernel", &flags, "x86_64-unknown-none")?;
            let smp = smp
                .or_else(|| std::thread::available_parallelism().ok().map(|x| x.get()))
                .unwrap_or(1);
            for test in tests {
                run(
                    &sh,
                    &Config {
                        kernel: test,
                        smp,
                        debug: false,
                        gdb,
                        args: &args,
                        test: true,
                    },
                )
                .context("test failed")?;
            }
            Ok(())
        }
    }
}
