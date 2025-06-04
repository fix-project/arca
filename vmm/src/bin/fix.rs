#![feature(allocator_api)]
#![feature(thread_sleep_until)]
#![feature(future_join)]

use std::process::Command;

use include_directory::{include_directory, Dir};
use vmm::client::*;

const MODULE_WAT: &str = r#"
(module
  (import "fixpoint" "create_blob_i32" (func $create_blob_i32 (param i32) (result externref)))
  (func $apply (param $arg externref) (result externref)
      ;; (local.get $arg))
      (call $create_blob_i32 (i32.const 7)))
  (export "_fixpoint_apply" (func $apply)))"#;
static WASM2C_RT: Dir<'_> = include_directory!("$CARGO_MANIFEST_DIR/wasm2c");

pub fn compile(wat: &[u8]) -> anyhow::Result<Vec<u8>> {
    let wasm = if &wat[..4] == b"\0asm" {
        wat
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
            ])
            .status()?;
        assert!(wat2wasm.success());
        &std::fs::read(wasm_file)?
    };
    let temp_dir = tempfile::tempdir()?;
    let mut wasm_file = temp_dir.path().to_path_buf();
    wasm_file.push("module.wasm");
    std::fs::write(&wasm_file, wasm)?;
    let mut c_file = temp_dir.path().to_path_buf();
    c_file.push("module.c");

    // Using wasm2c 1.0.34 from the Ubuntu repos
    let wasm2c = Command::new("wasm2c")
        .args([
            "-o",
            c_file.to_str().unwrap(),
            "-n",
            "module",
            wasm_file.to_str().unwrap(),
        ])
        .status()?;
    assert!(wasm2c.success());
    WASM2C_RT.extract(&temp_dir)?;

    let mut o_file = temp_dir.path().to_path_buf();
    o_file.push("module.o");

    let mut start = temp_dir.path().to_path_buf();
    start.push("start.S");

    let mut lib = temp_dir.path().to_path_buf();
    lib.push("lib.c");

    let mut memmap = temp_dir.path().to_path_buf();
    memmap.push("memmap.ld");

    let mut wasm_rt_impl = temp_dir.path().to_path_buf();
    wasm_rt_impl.push("wasm-rt-impl.c");

    let cc = Command::new("clang")
        .args([
            "-target",
            "x86_64-unknown-none",
            "-o",
            o_file.to_str().unwrap(),
            "-I",
            temp_dir.path().to_str().unwrap(),
            start.to_str().unwrap(),
            c_file.to_str().unwrap(),
            lib.to_str().unwrap(),
            wasm_rt_impl.to_str().unwrap(),
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
            "-Wl,-no-pie",
        ])
        .status()?;
    assert!(cc.success());

    let o = std::fs::read(o_file)?;

    Ok(o)
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let elf = compile(MODULE_WAT.as_bytes())?;
    let arca = vmm::client::runtime();

    log::info!("create blob");
    let elf = arca.create_blob(&elf);
    log::info!("create thunk");
    let thunk: Ref<Thunk> = elf.into_thunk();
    log::info!("run thunk");
    let lambda: Ref<Lambda> = thunk.run().try_into().unwrap();
    let thunk = lambda.apply(arca.create_word(0xcafeb0ba).into());
    let word: Ref<Word> = thunk.run().try_into().unwrap();
    log::info!("{:?}", word.read());

    Ok(())
}
