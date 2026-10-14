use std::{
    env, fs,
    io::{self, Read, Write},
};

fn main() {
    assert_eq!(env::args().skip(1).collect::<Vec<_>>(), ["argument"]);
    assert_eq!(env::var("GREETING").unwrap(), "hello");
    assert_eq!(fs::read("/work/input.txt").unwrap(), b"original");
    fs::rename("/work/input.txt", "/work/renamed.txt").unwrap();
    fs::write("/work/result", b"changed\0bytes").unwrap();
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input).unwrap();
    io::stdout().write_all(&input).unwrap();
    eprintln!("guest diagnostic");
    std::process::exit(23);
}
