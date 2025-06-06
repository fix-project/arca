use std::env;
use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

use anyhow::Result;

use include_directory::{Dir, include_directory};

static FIX_SHELL: Dir<'_> = include_directory!("$CARGO_MANIFEST_DIR/fix-shell");
static INC: Dir<'_> = include_directory!("$CARGO_MANIFEST_DIR/../defs/arca");
static SRC: Dir<'_> = include_directory!("$CARGO_MANIFEST_DIR/../defs/src");

fn wat2wasm(wat: &[u8]) -> Result<Vec<u8>> {
    if &wat[..4] == b"\0asm" {
        Ok(wat.into())
    } else {
        let temp_dir = tempfile::tempdir()?;
        let mut wat_file = temp_dir.path().to_path_buf();
        wat_file.push("module.wat");
        std::fs::write(&wat_file, wat)?;
        let mut wasm_file = temp_dir.path().to_path_buf();
        wasm_file.push("module.wasm");
        let wat2wasm = Command::new("wat2wasm")
            .args([
                "-o",
                wasm_file.to_str().unwrap(),
                wat_file.to_str().unwrap(),
                "--enable-multi-memory",
            ])
            .status()?;
        assert!(wat2wasm.success());
        Ok(std::fs::read(wasm_file)?)
    }
}

fn wasm2c(wasm: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let temp_dir = tempfile::tempdir()?;
    let mut wasm_file = temp_dir.path().to_path_buf();
    wasm_file.push("module.wasm");
    std::fs::write(&wasm_file, wasm)?;
    let mut c_file = temp_dir.path().to_path_buf();
    c_file.push("module.c");
    let mut h_file = temp_dir.path().to_path_buf();
    h_file.push("module.h");

    // Using wasm2c 1.0.34 from the Ubuntu repos
    let wasm2c = Command::new("wasm2c")
        .args([
            "-o",
            c_file.to_str().unwrap(),
            "-n",
            "module",
            wasm_file.to_str().unwrap(),
            "--enable-multi-memory",
        ])
        .status()?;
    assert!(wasm2c.success());
    Ok((std::fs::read(c_file)?, std::fs::read(h_file)?))
}

fn c2elf(c: &[u8], h: &[u8]) -> Result<Vec<u8>> {
    let temp_dir = tempfile::tempdir()?;
    FIX_SHELL.extract(&temp_dir)?;
    INC.extract(&temp_dir)?;
    SRC.extract(&temp_dir)?;

    let mut c_file = temp_dir.path().to_path_buf();
    c_file.push("module.c");

    let mut h_file = temp_dir.path().to_path_buf();
    h_file.push("module.h");

    std::fs::write(c_file, c)?;
    std::fs::write(h_file, h)?;

    let mut src = vec![];
    let exts = [OsStr::new("c"), OsStr::new("S")];
    for f in std::fs::read_dir(&temp_dir)? {
        let f = f?;
        if let Some(ext) = f.path().extension() {
            if exts.contains(&ext) {
                src.push(f.path());
            }
        }
    }

    println!("{:?}", src);

    let mut o_file = temp_dir.path().to_path_buf();
    o_file.push("module.o");

    let mut memmap = temp_dir.path().to_path_buf();
    memmap.push("memmap.ld");

    let cc = Command::new("clang")
        .args([
            "-target",
            "x86_64-unknown-none",
            "-o",
            o_file.to_str().unwrap(),
            "-I",
            temp_dir.path().to_str().unwrap(),
            "-T",
            memmap.to_str().unwrap(),
            // "-lm",
            "-O2",
            "-fno-optimize-sibling-calls",
            "-frounding-math",
            // "-fsignaling-nans",
            "-ffreestanding",
            "-nostdlib",
            "-nostartfiles",
            "-mcmodel=large",
            "-Wl,-no-pie",
        ])
        .args(src)
        .status()?;
    assert!(cc.success());

    let o = std::fs::read(o_file)?;

    Ok(o)
}

fn main() -> Result<()> {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    for f in std::fs::read_dir("wasm")? {
        let f = f?;
        let path = f.path();
        let base = path.file_stem().unwrap();
        let dst = Path::new(&out_dir).join(base);
        println!(
            "cargo::rerun-if-changed=wasm/{}",
            f.file_name().to_string_lossy()
        );
        let wat = std::fs::read(f.path())?;
        let wasm = wat2wasm(&wat)?;
        let (c, h) = wasm2c(&wasm)?;
        let elf = c2elf(&c, &h)?;
        std::fs::write(dst, elf)?;
    }
    Ok(())
}
