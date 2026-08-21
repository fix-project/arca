use std::os::unix::fs::symlink;
use std::path::Path;
use std::{env, fs};

use anyhow::Result;

fn main() -> Result<()> {
    let out_dir = env::var_os("OUT_DIR").unwrap();

    for &(name, elf) in fix_wasm::ARTIFACTS {
        let dst = Path::new(&out_dir).join(name);
        std::fs::write(&dst, elf)?;

        let link = Path::new(&out_dir).ancestors().nth(4).unwrap().join(name);
        let _ = fs::remove_file(&link);
        symlink(dst, link)?;
    }

    let cwd = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rerun-if-changed={cwd}/etc/memmap.ld");
    println!("cargo::rustc-link-arg=-T{cwd}/etc/memmap.ld");
    println!("cargo::rustc-link-arg=-no-pie");

    Ok(())
}
