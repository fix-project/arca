use std::env;
use std::ffi::OsStr;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use anyhow::{Result, anyhow};
use cmake::Config;

use include_directory::{Dir, include_directory};

static FIX_SHELL: Dir<'_> = include_directory!("$CARGO_MANIFEST_DIR/fix-shell");
static INC: Dir<'_> = include_directory!("$CARGO_MANIFEST_DIR/../defs/arca");
static SRC: Dir<'_> = include_directory!("$CARGO_MANIFEST_DIR/../defs/src");

static WASM2C: OnceLock<PathBuf> = OnceLock::new();
static WAT2WASM: OnceLock<PathBuf> = OnceLock::new();

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
        let wat2wasm = Command::new(WAT2WASM.get().unwrap())
            .args([
                "-o",
                wasm_file.to_str().unwrap(),
                wat_file.to_str().unwrap(),
                "--enable-multi-memory",
            ])
            .status()
            .map_err(|e| {
                if let ErrorKind::NotFound = e.kind() {
                    anyhow!("Could not find wat2wasm. Did you install wabt?")
                } else {
                    e.into()
                }
            })?;
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
    let wasm2c = Command::new(WASM2C.get().unwrap())
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
        if let Some(ext) = f.path().extension()
            && exts.contains(&ext)
        {
            src.push(f.path());
        }
    }

    println!("{src:?}");

    let mut o_file = temp_dir.path().to_path_buf();
    o_file.push("module.o");

    let mut memmap = temp_dir.path().to_path_buf();
    memmap.push("memmap.ld");

    let cc = Command::new("clang")
        .args([
            "-target",
            "x86_64-unknown-none", // TODO: modify wasm2c to not require non-freestanding libraries (e.g., <math.h>)
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
            "-Imath.h",
        ])
        .args(src)
        .status().map_err(|e| if let ErrorKind::NotFound = e.kind() {anyhow!("Compilation failed. Please make sure you have installed gcc-multilib if you are on Ubuntu.")} else {e.into()})?;
    assert!(cc.success());

    let o = std::fs::read(o_file)?;

    Ok(o)
}

fn main() -> Result<()> {
    let dst = Config::new("wabt")
        .define("BUILD_TESTS", "OFF")
        .define("BUILD_LIBWASM", "OFF")
        .define("BUILD_TOOLS", "ON")
        .build();

    WASM2C.set(dst.join("bin/wasm2c")).unwrap();
    WAT2WASM.set(dst.join("bin/wat2wasm")).unwrap();

    let out_dir = env::var_os("OUT_DIR").unwrap();
    for f in std::fs::read_dir("wasm")? {
        let f = f?;
        let path = f.path();
        let base = path.file_stem().unwrap();
        let dst = Path::new(&out_dir).join(base);
        println!(
            "cargo::rerun-if-changed=wasm/{}",
            f.file_name().to_str().unwrap()
        );
        let wat = std::fs::read(f.path())?;
        let wasm = wat2wasm(&wat)?;
        let (c, h) = wasm2c(&wasm)?;
        let elf = c2elf(&c, &h)?;
        std::fs::write(dst, elf)?;
    }
    Ok(())
}
