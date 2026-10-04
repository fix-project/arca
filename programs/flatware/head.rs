use std::{
    env,
    ffi::OsString,
    fs::File,
    io::{self, BufRead, BufReader, Write},
};

fn head(input: impl io::Read, output: &mut impl Write) -> io::Result<()> {
    let mut input = BufReader::new(input);
    let mut line = Vec::new();
    for _ in 0..10 {
        line.clear();
        if input.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        output.write_all(&line)?;
    }
    Ok(())
}

fn main() {
    let mut paths: Vec<_> = env::args_os().skip(1).collect();
    if paths.is_empty() {
        paths.push(OsString::from("-"));
    }
    let mut output = io::stdout().lock();
    let mut failed = false;
    for path in paths {
        let result = if path == "-" {
            head(io::stdin().lock(), &mut output)
        } else {
            File::open(&path).and_then(|file| head(file, &mut output))
        };
        if let Err(error) = result {
            eprintln!("head: {}: {error}", path.to_string_lossy());
            failed = true;
        }
    }
    if failed {
        std::process::exit(1);
    }
}
