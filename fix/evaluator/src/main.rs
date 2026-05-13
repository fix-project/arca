#![feature(allocator_api)]
#![feature(ptr_metadata)]

use crate::fixruntime::{CouponHelper, DeterministicEquivRuntime};
use common::bitpack::BitPack;
use fixhandle::rawhandle::{create_application_thunk, create_strict_encode};
use std::path::PathBuf;

use crate::vmmruntime::VmmRuntime;
use clap::Parser;

mod fixruntime;
mod vmmruntime;

#[derive(Parser, Debug)]
struct Args {
    module: PathBuf,
    kernel: PathBuf,
    #[arg(short, long)]
    smp: Option<usize>,
    #[arg(short, long, default_value = "3")]
    cid: usize,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();
    let smp = args
        .smp
        // .or_else(|| std::thread::available_parallelism().ok().map(|x| x.get()))
        .unwrap_or(1);
    let cid = args.cid;

    let bin = std::fs::read(args.kernel)?;
    let module = std::fs::read(args.module)?;

    let mut rt = VmmRuntime::new(smp, cid, bin);

    let function = rt.create_blob(module.as_slice());
    let addend1 = rt.create_blob_i64(3);
    let addend2 = rt.create_blob_i64(4);
    let addend3 = rt.create_blob_i64(1024);

    let mut scratch = Vec::with_capacity(3 * 32);
    scratch.extend_from_slice(&function.pack());
    scratch.extend_from_slice(&addend1.pack());
    scratch.extend_from_slice(&addend2.pack());

    let combination = rt.create_tree(scratch.as_slice());
    let application = create_application_thunk(&combination).unwrap();
    let encode = create_strict_encode(&application).unwrap();

    let mut scratch = Vec::with_capacity(3 * 32);
    scratch.extend_from_slice(&function.pack());
    scratch.extend_from_slice(&encode.pack());
    scratch.extend_from_slice(&addend3.pack());

    let combination = rt.create_tree(scratch.as_slice());
    let application = create_application_thunk(&combination).unwrap();
    let encode = create_strict_encode(&application).unwrap();

    let result = rt.eval(encode);
    rt.show_coupon(&result);

    let result_blob = rt.get_coupon_rhs(&result);
    let result_blob = rt.get_blob(&result_blob).expect("Result is not a Blob");
    let mut arr = [0u8; 8];
    arr.copy_from_slice(result_blob);
    let num = u64::from_le_bytes(arr);
    log::info!("{:?}", num);
    assert_eq!(num, 1031);

    Ok(())
}
