use runfiles::{Runfiles, rlocation};
use std::{
    env,
    io::Write,
    process::{Command, Stdio},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runfiles = Runfiles::create()?;
    let fix = rlocation!(runfiles, env!("FIX_CLI")).ok_or("missing Fix CLI")?;
    let program = rlocation!(runfiles, env!("FIX_PROGRAM")).ok_or("missing program")?;
    let mut command = Command::new(&fix);
    let args = env::args_os().skip(1);
    let mut source = format!(
        "program = @file(\"{}\")\n*#(\"\" program",
        program.display()
    );
    for argument in args {
        source.push(' ');
        source.push_str(
            argument
                .to_str()
                .ok_or("arguments must be Fix expressions")?,
        );
    }
    source.push_str(")\n");
    let mut child = command.args(["run", "-"]).stdin(Stdio::piped()).spawn()?;
    child.stdin.take().unwrap().write_all(source.as_bytes())?;
    std::process::exit(child.wait()?.code().unwrap_or(1));
}
