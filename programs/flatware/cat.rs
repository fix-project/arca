use std::{
    env,
    ffi::OsString,
    fs::File,
    io::{self, Write},
};

fn main() {
    let mut paths: Vec<_> = env::args_os().skip(1).collect();
    if paths.is_empty() {
        paths.push(OsString::from("-"));
    }
    let mut failed = false;
    let mut output = io::stdout().lock();
    for path in paths {
        let result = if path == "-" {
            io::copy(&mut io::stdin().lock(), &mut output)
        } else {
            File::open(&path).and_then(|mut file| io::copy(&mut file, &mut output))
        };
        if let Err(error) = result {
            eprintln!("cat: {}: {error}", path.to_string_lossy());
            failed = true;
        }
    }
    if let Err(error) = output.flush() {
        eprintln!("cat: {error}");
        failed = true;
    }
    if failed {
        std::process::exit(1);
    }
}
