use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use anyhow::Result;
use clap::{Parser, Subcommand};
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

fn run(
    sh: &Shell,
    loader: &Path,
    kernel: &Path,
    smp: usize,
    debug: bool,
    gdb: bool,
    args: &[String],
) -> Result<()> {
    let loader = loader.display().to_string();
    let kernel = kernel.display().to_string();
    let smp = smp.to_string();

    let qemu = cmd!(sh, "qemu-kvm -cpu host,+invtsc,+vmware-cpuid-freq -machine microvm -enable-kvm -monitor none -serial none -debugcon stdio -nographic -no-reboot -smp {smp} -m 4G -bios /usr/share/qemu/qboot.rom -kernel {loader} -device loader,file={kernel}");

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

    Ok(qemu.run()?)
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
            build(&sh, "loader", &args, "i686-unknown-none")?;
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
            let loader = &build(&sh, "loader", &flags, "i686-unknown-none")?[0];
            let kernel = &build(&sh, "kernel", &flags, "x86_64-unknown-none")?[0];
            let smp = smp
                .or_else(|| std::thread::available_parallelism().ok().map(|x| x.get()))
                .unwrap_or(1);
            run(&sh, loader, kernel, smp, debug, gdb, &args)
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
            let loader = &build(&sh, "loader", &flags, "i686-unknown-none")?[0];
            flags.push("--tests");
            let tests = &build(&sh, "kernel", &flags, "x86_64-unknown-none")?;
            let smp = smp
                .or_else(|| std::thread::available_parallelism().ok().map(|x| x.get()))
                .unwrap_or(1);
            for test in tests {
                run(&sh, loader, test, smp, true, gdb, &args)?;
            }
            Ok(())
        }
    }
}
