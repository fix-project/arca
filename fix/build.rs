use std::env;
use std::fs::create_dir_all;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use anyhow::{Result, anyhow};
use cmake::Config;

use include_directory::{Dir, include_directory};

static FIX_SHELL_INC: Dir<'_> = include_directory!("$CARGO_MANIFEST_DIR/shell/inc");
static FIX_SHELL_ETC: Dir<'_> = include_directory!("$CARGO_MANIFEST_DIR/shell/etc");

static INTERMEDIATEOUT: OnceLock<PathBuf> = OnceLock::new();
static WASM2C: OnceLock<PathBuf> = OnceLock::new();
static WAT2WASM: OnceLock<PathBuf> = OnceLock::new();

fn wat2wasm(wat: &[u8]) -> Result<Vec<u8>> {
    if &wat[..4] == b"\0asm" {
        Ok(wat.into())
    } else {
        let mut wat_file = INTERMEDIATEOUT.get().unwrap().clone();
        wat_file.push("module.wat");
        std::fs::write(&wat_file, wat)?;
        let mut wasm_file = INTERMEDIATEOUT.get().unwrap().clone();
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
    let mut wasm_file = INTERMEDIATEOUT.get().unwrap().clone();
    wasm_file.push("module.wasm");
    std::fs::write(&wasm_file, wasm)?;
    let mut c_file = INTERMEDIATEOUT.get().unwrap().clone();
    c_file.push("module.c");
    let mut h_file = INTERMEDIATEOUT.get().unwrap().clone();
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
    FIX_SHELL_INC.extract(INTERMEDIATEOUT.get().unwrap())?;
    FIX_SHELL_ETC.extract(INTERMEDIATEOUT.get().unwrap())?;

    let mut wasm_rt = INTERMEDIATEOUT.get().unwrap().clone();
    wasm_rt.push("wasm-rt.c");

    let mut c_file = INTERMEDIATEOUT.get().unwrap().clone();
    c_file.push("module.c");

    let mut h_file = INTERMEDIATEOUT.get().unwrap().clone();
    h_file.push("module.h");

    std::fs::write(c_file.clone(), c)?;
    std::fs::write(h_file, h)?;

    let mut src = vec![c_file, wasm_rt];

    let shell_top = env::var_os("CARGO_STATICLIB_FILE_FIXSHELL_fixshell").unwrap();
    src.push(PathBuf::from(shell_top));

    println!("{src:?}");

    let mut o_file = INTERMEDIATEOUT.get().unwrap().clone();
    o_file.push("module.o");

    let mut memmap = INTERMEDIATEOUT.get().unwrap().clone();
    memmap.push("memmap.ld");

    let cc = Command::new("gcc")
        .args([
            "-o",
            o_file.to_str().unwrap(),
            "-T",
            memmap.to_str().unwrap(),
            "-O2",
            "-fno-optimize-sibling-calls",
            "-frounding-math",
            // "-fsignaling-nans",
            "-ffreestanding",
            "-nostdlib",
            "-nostartfiles",
            "--verbose",
            "-mcmodel=large",
            // "-fno-pic",
            // "-fno-pie",
            // "-Wl,-no-pie",
            // "-static",
        ])
        .args(src)
        .status().map_err(|e| if let ErrorKind::NotFound = e.kind() {anyhow!("Compilation failed. Please make sure you have installed gcc-multilib if you are on Ubuntu.")} else {e.into()})?;
    assert!(cc.success());

    let o = std::fs::read(o_file)?;

    Ok(o)
}

fn main() -> Result<()> {
    let out_dir = env::var_os("OUT_DIR").unwrap();

    let mut intermediateout: PathBuf = out_dir.clone().into();
    intermediateout.push("inter-out");
    if intermediateout.exists() {
        std::fs::remove_dir_all(&intermediateout)?;
    }
    create_dir_all(&intermediateout)?;
    INTERMEDIATEOUT.set(intermediateout).unwrap();

    let mut dst: PathBuf = out_dir.clone().into();
    dst.push("wabt");
    if !dst.exists() {
        create_dir_all(&dst)?
    }

    let dst = Config::new("wabt")
        .define("BUILD_TESTS", "OFF")
        .define("BUILD_LIBWASM", "OFF")
        .define("BUILD_TOOLS", "ON")
        .out_dir(dst)
        .build();

    WASM2C.set(dst.join("bin/wasm2c")).unwrap();
    WAT2WASM.set(dst.join("bin/wat2wasm")).unwrap();

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

    let cwd = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    println!("cargo::rerun-if-changed={cwd}/etc/memmap.ld");
    println!("cargo::rustc-link-arg=-T{cwd}/etc/memmap.ld");
    println!("cargo::rustc-link-arg=-no-pie");

    Ok(())
}
