use std::{
    env,
    ffi::OsString,
    fs::File,
    io::{self, Read},
};

fn count(mut input: impl Read) -> io::Result<[u64; 3]> {
    let mut counts = [0; 3];
    let mut in_word = false;
    let mut buffer = [0; 8192];
    loop {
        let length = input.read(&mut buffer)?;
        if length == 0 {
            return Ok(counts);
        }
        counts[2] += length as u64;
        for &byte in &buffer[..length] {
            counts[0] += u64::from(byte == b'\n');
            let word = !byte.is_ascii_whitespace();
            counts[1] += u64::from(word && !in_word);
            in_word = word;
        }
    }
}

fn main() {
    let mut paths: Vec<_> = env::args_os().skip(1).collect();
    if paths.is_empty() {
        paths.push(OsString::from("-"));
    }
    let mut total = [0; 3];
    let mut failed = false;
    for path in &paths {
        let result = if path == "-" {
            count(io::stdin().lock())
        } else {
            File::open(path).and_then(count)
        };
        match result {
            Ok(counts) => {
                println!(
                    "{} {} {} {}",
                    counts[0],
                    counts[1],
                    counts[2],
                    path.to_string_lossy()
                );
                for (sum, count) in total.iter_mut().zip(counts) {
                    *sum += count;
                }
            }
            Err(error) => {
                eprintln!("wc: {}: {error}", path.to_string_lossy());
                failed = true;
            }
        }
    }
    if paths.len() > 1 {
        println!("{} {} {} total", total[0], total[1], total[2]);
    }
    if failed {
        std::process::exit(1);
    }
}
