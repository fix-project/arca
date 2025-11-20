use std::env;
use std::ffi::OsStr;
use std::fs::create_dir_all;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use anyhow::{Result, anyhow};
use cmake::Config;

use include_directory::{Dir, include_directory};

static FIX_SHELL: Dir<'_> = include_directory!("$CARGO_MANIFEST_DIR/wasi-shell");

static INTERMEDIATEOUT: OnceLock<PathBuf> = OnceLock::new();
static ARCAPREFIX: OnceLock<PathBuf> = OnceLock::new();
static WASM2C: OnceLock<PathBuf> = OnceLock::new();
static WAT2WASM: OnceLock<PathBuf> = OnceLock::new();

fn c2wasm(c: &[u8]) -> Result<Vec<u8>> {
    if &c[..4] == b"\0asm" {
        Ok(c.into())
    } else {
        let mut c_file = INTERMEDIATEOUT.get().unwrap().clone();
        c_file.push("module.c");
        std::fs::write(&c_file, c)?;
        let mut wasm_file = INTERMEDIATEOUT.get().unwrap().clone();
        wasm_file.push("module.wasm");

        let (program, extra_args) = if Path::new("/opt/wasi-sdk/bin/clang").exists() {
            (String::from("/opt/wasi-sdk/bin/clang"), vec![])
        } else {
            let wasi_sdk = std::env::var("WASI_SDK_PATH")
                .map_err(|_| anyhow!("Could not WASI_SDK_PATH. Did you install wasi-sdk?"))?;

            let program = {
                let mut p = PathBuf::from(wasi_sdk.clone());
                p.push("bin/clang");
                if p.exists() {
                    Ok(String::from(p.to_str().expect("Invalid path")))
                } else {
                    Err(anyhow!(
                        "Could not find wasi clang. Did you install wasi-sdk?"
                    ))
                }
            }?;

            (
                program,
                vec![format!("--sysroot={}/share/wasi-sysroot", wasi_sdk)],
            )
        };
        println!("{} {:?}", program, extra_args);
        let clang = Command::new(program)
            .args(extra_args)
            .args(["-o", wasm_file.to_str().unwrap(), c_file.to_str().unwrap()])
            .status()
            .map_err(Into::<anyhow::Error>::into)?;
        assert!(clang.success());
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
    FIX_SHELL.extract(INTERMEDIATEOUT.get().unwrap())?;

    let mut c_file = INTERMEDIATEOUT.get().unwrap().clone();
    c_file.push("module.c");

    let mut h_file = INTERMEDIATEOUT.get().unwrap().clone();
    h_file.push("module.h");

    std::fs::write(c_file, c)?;
    std::fs::write(h_file, h)?;

    let mut src = vec![];
    let exts = [OsStr::new("c"), OsStr::new("S")];
    for f in std::fs::read_dir(INTERMEDIATEOUT.get().unwrap())? {
        let f = f?;
        if let Some(ext) = f.path().extension()
            && exts.contains(&ext)
        {
            src.push(f.path());
        }
    }

    println!("{src:?}");

    let mut o_file = INTERMEDIATEOUT.get().unwrap().clone();
    o_file.push("module.o");

    let mut memmap = INTERMEDIATEOUT.get().unwrap().clone();
    memmap.push("memmap.ld");

    let prefix = ARCAPREFIX.get().unwrap();
    let gcc = prefix.join("bin/musl-gcc");

    let cc = Command::new(gcc)
        .args([
            "-Wl,-static",
            "-no-pie",
            "-o",
            o_file.to_str().unwrap(),
            "-O2",
            "-fno-optimize-sibling-calls",
            "-frounding-math",
            "-fsignaling-nans",
            //"-Twasi-shell/memmap.ld",
            // "--verbose",
            "-Wl,-no-pie",
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
    if !intermediateout.exists() {
        create_dir_all(&intermediateout)?
    }
    INTERMEDIATEOUT.set(intermediateout).unwrap();

    let mut prefix: PathBuf = out_dir.clone().into();
    prefix.push("arca-musl");

    if !prefix.exists() {
        create_dir_all(&prefix)?
    }

    let prefix = autotools::Config::new("../modules/arca-musl")
        .disable_shared()
        .enable_static()
        .out_dir(prefix)
        .build();

    ARCAPREFIX.set(prefix).unwrap();

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
        let c = std::fs::read(f.path())?;
        let wasm = c2wasm(&c)?;
        let (c, h) = wasm2c(&wasm)?;
        let elf = c2elf(&c, &h)?;
        std::fs::write(dst, elf)?;
    }

    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rerun-if-changed={dir}/etc/memmap.ld");
    println!("cargo::rustc-link-arg=-T{dir}/etc/memmap.ld");
    println!("cargo::rustc-link-arg=-no-pie");

    Ok(())
}
