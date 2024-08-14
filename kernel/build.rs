use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use glob::{glob, GlobError};
use nasm_rs as nasm;

fn main() -> Result<()> {
    let asm = glob("src/**/*.asm").context("failed to read glob")?;
    let asm: Result<Vec<PathBuf>, GlobError> = asm.collect();
    let asm = asm.context("could not find asm files")?;
    for file in &asm {
        println!("cargo::rerun-if-changed=src/{}", file.display());
    }
    nasm::Build::new()
        .files(&asm)
        .compile("kernel-asm")
        .map_err(|x| anyhow!("could not assemble: {x}"))?;
    println!("cargo::rustc-link-lib=static=kernel-asm");

    cc::Build::new().file("src/start.S").compile("start");
    println!("cargo::rerun-if-changed=src/start.S");
    cc::Build::new()
        .file("src/interrupts.S")
        .compile("interrupts");
    println!("cargo::rerun-if-changed=src/interrupts.S");

    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rerun-if-changed=etc/memmap.ld");
    println!("cargo::rustc-link-arg=-T{}/etc/memmap.ld", dir);
    println!("cargo::rustc-link-arg=-no-pie");
    Ok(())
}
