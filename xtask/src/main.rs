#![feature(lazy_cell)]
use std::{
    path::PathBuf,
    sync::{LazyLock, OnceLock},
};

use anyhow::Result;
use clap::{Parser, Subcommand};
use xshell::{cmd, Shell};

static CARGO: LazyLock<String> =
    LazyLock::new(|| std::env::var("CARGO").unwrap_or("cargo".to_string()));
static WORKSPACE: OnceLock<PathBuf> = OnceLock::new();

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
        /// Build and run in release mode
        #[clap(long, default_value_t = false)]
        release: bool,
    },
    Run {
        /// Build and run in release mode
        #[clap(long, default_value_t = false)]
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
    },
}

fn build(sh: &Shell, package: &str, release: bool, target: &str) -> Result<PathBuf> {
    let profile = if release { "release" } else { "dev" };
    let cargo: &str = &CARGO;
    cmd!(
        sh,
        "{cargo} build -p {package} --profile {profile} --target {target}"
    )
    .run()?;
    Ok(if release {
        WORKSPACE
            .get()
            .unwrap()
            .join(format!("target/{target}/release/{package}"))
    } else {
        WORKSPACE
            .get()
            .unwrap()
            .join(format!("target/{target}/debug/{package}"))
    })
}

fn main() -> Result<()> {
    let sh = Shell::new()?;
    let cargo: &str = &CARGO;
    let args = Args::parse();

    WORKSPACE
        .set({
            let info = cmd!(sh, "{cargo} locate-project --workspace").read()?;
            let info = json::parse(&info)?;
            let root = &info["root"];
            PathBuf::from(root.as_str().expect("could not find workspace root"))
                .parent()
                .unwrap()
                .to_path_buf()
        })
        .unwrap();

    match args.command {
        SubCommand::Build { release } => {
            build(&sh, "kernel", release, "x86_64-unknown-none")?;
            build(&sh, "loader", release, "i686-unknown-none")?;
            Ok(())
        }
        SubCommand::Run {
            release,
            smp,
            debug,
            gdb,
        } => {
            let kernel = build(&sh, "kernel", release, "x86_64-unknown-none")?
                .display()
                .to_string();
            let loader = build(&sh, "loader", release, "i686-unknown-none")?
                .display()
                .to_string();
            let smp = smp
                .or_else(|| std::thread::available_parallelism().ok().map(|x| x.get()))
                .unwrap_or(1)
                .to_string();

            let qemu = 
            cmd!(sh, "qemu-kvm -machine microvm -monitor none -serial none -debugcon stdio -nographic -no-reboot -smp {smp} -m 4G -bios /usr/share/qemu/qboot.rom -kernel {loader} -device loader,file={kernel}");

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
            Ok(qemu.run()?)
        }
    }
}
