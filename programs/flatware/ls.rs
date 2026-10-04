use std::{
    env, fs,
    io::{self, Write},
    path::PathBuf,
};

fn main() {
    let mut paths: Vec<_> = env::args_os().skip(1).map(PathBuf::from).collect();
    if paths.is_empty() {
        paths.push(PathBuf::from("."));
    }
    let mut failed = false;
    let mut output = io::stdout().lock();
    for path in paths {
        let result = (|| -> io::Result<()> {
            if path.is_dir() {
                let mut names = fs::read_dir(&path)?
                    .map(|entry| entry.map(|entry| entry.file_name()))
                    .collect::<io::Result<Vec<_>>>()?;
                names.sort();
                for name in names {
                    writeln!(output, "{}", name.to_string_lossy())?;
                }
            } else {
                fs::metadata(&path)?;
                writeln!(output, "{}", path.display())?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("ls: {}: {error}", path.display());
            failed = true;
        }
    }
    if failed {
        std::process::exit(1);
    }
}
