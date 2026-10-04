use std::{
    env, fs,
    io::{self, Read, Seek, SeekFrom, Write},
};

fn main() {
    assert_eq!(env::args().skip(1).collect::<Vec<_>>(), ["argument"]);
    assert_eq!(env::var("GREETING").unwrap(), "hello");
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input).unwrap();
    assert_eq!(input, b"buffered\0input");
    assert_eq!(fs::read("/work/input.txt").unwrap(), b"original");
    for path in ["/work/dangling", "/work/link"] {
        assert!(fs::symlink_metadata(path).unwrap().is_symlink());
        assert_eq!(fs::metadata(path).unwrap_err().raw_os_error(), Some(58));
    }
    let before = std::time::SystemTime::now();
    fs::create_dir("/work/nested").unwrap();
    fs::write("/work/nested/result", b"changed\0bytes").unwrap();
    let mut file = fs::File::open("/work/nested/result").unwrap();
    file.seek(SeekFrom::Start(7)).unwrap();
    let mut tail = Vec::new();
    file.read_to_end(&mut tail).unwrap();
    assert_eq!(tail, b"\0bytes");
    assert!(file.metadata().unwrap().modified().unwrap() > before);
    assert_eq!(fs::read_dir("/work/nested").unwrap().count(), 1);
    fs::rename("/work/input.txt", "/work/renamed.txt").unwrap();
    fs::rename("/work/source", "/work/nested/moved").unwrap();
    fs::rename("/work/dangling", "/work/nested/moved/dangling").unwrap();
    io::stdout().write_all(&input).unwrap();
    eprintln!("guest diagnostic");
    std::process::exit(23);
}
