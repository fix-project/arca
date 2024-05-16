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
        /// Whether to build and run in release mode
        #[clap(long, default_value_t = false)]
        release: bool,
    },
    Run {
        /// Whether to build and run in release mode
        #[clap(long, default_value_t = false)]
        release: bool,
        /// The number of vCPUs to use (defaults to the number of hardware CPUs).
        smp: Option<usize>,
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
        SubCommand::Run { release, smp } => {
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

            Ok(cmd!(sh, "qemu-kvm -machine microvm -monitor none -serial stdio -nographic -no-reboot -smp {smp} -m 4G -bios /usr/share/qemu/qboot.rom -kernel {loader} -device loader,file={kernel}").run()?)
        }
    }
}
