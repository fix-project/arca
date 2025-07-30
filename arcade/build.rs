use std::{env, io::ErrorKind, path::PathBuf, process::Command};

use anyhow::{Result, anyhow};

fn c2elf() -> Result<()> {
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    let out = out_path.join("foo");
    let cc = Command::new("/opt/arca/musl/bin/musl-gcc")
        .args([
            "-o",
            out.to_str().unwrap(),
            "-static",
        ])
        .arg("foo.c")
        .status().map_err(|e| if let ErrorKind::NotFound = e.kind() {anyhow!("Compilation failed. Please make sure you have installed gcc-multilib if you are on Ubuntu.")} else {e.into()})?;
    assert!(cc.success());
    println!("cargo::rerun-if-changed=foo.c",);
    println!("cargo::rerun-if-changed=/opt/arca",);
    Ok(())
}

fn main() -> Result<()> {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rerun-if-changed={dir}/etc/memmap.ld");
    println!("cargo::rustc-link-arg=-T{dir}/etc/memmap.ld");
    println!("cargo::rustc-link-arg=-no-pie");

    c2elf()?;

    Ok(())
}
