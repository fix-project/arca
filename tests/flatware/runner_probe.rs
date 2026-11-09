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
    if matches!(env::args().nth(1).as_deref(), Some("read" | "set-atime")) {
        let set_atime = env::args().nth(1).as_deref() == Some("set-atime");
        let mut file = fs::File::options()
            .read(true)
            .write(set_atime)
            .open("/work/input.txt")
            .unwrap();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"original");
        let metadata = file.metadata().unwrap();
        assert_eq!(metadata.len(), 8);
        assert_eq!(
            metadata.accessed().unwrap(),
            UNIX_EPOCH + std::time::Duration::from_nanos(17)
        );
        if set_atime {
            file.set_times(
                fs::FileTimes::new().set_accessed(UNIX_EPOCH + std::time::Duration::from_nanos(29)),
            )
            .unwrap();
        }
        return;
    }
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
