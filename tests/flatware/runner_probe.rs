#![feature(wasi_ext)]
use std::{
    env, fs,
    io::{self, Read, Write},
    os::wasi::fs::MetadataExt,
    time::{SystemTime, UNIX_EPOCH},
};

#[link(wasm_import_module = "wasi_snapshot_preview1")]
unsafe extern "C" {
    fn random_get(bytes: *mut u8, len: u32) -> u32;
}

fn main() {
    if env::args().nth(1).as_deref() == Some("copy") {
        let args: Vec<_> = env::args().collect();
        fs::write(&args[3], fs::read(&args[2]).unwrap()).unwrap();
        return;
    }
    assert_eq!(env::args().skip(1).collect::<Vec<_>>(), ["argument"]);
    assert_eq!(env::var("GREETING").unwrap(), "hello");
    let metadata = fs::metadata("/work/input.txt").unwrap();
    let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let mut random = [0; 16];
    assert_eq!(unsafe { random_get(random.as_mut_ptr(), 16) }, 0);
    fs::write(
        "/work/settings",
        format!(
            "{time:?} {:?} {} {random:?}",
            metadata.modified().unwrap(),
            metadata.nlink()
        ),
    )
    .unwrap();
    assert_eq!(fs::read("/work/input.txt").unwrap(), b"original");
    fs::rename("/work/input.txt", "/work/renamed.txt").unwrap();
    fs::write("/work/result", b"changed\0bytes").unwrap();
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input).unwrap();
    io::stdout().write_all(&input).unwrap();
    eprintln!("guest diagnostic");
    std::process::exit(23);
}
