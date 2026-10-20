use runfiles::{Runfiles, rlocation};
use std::{
    env,
    fs::File,
    io::Write,
    os::unix::process::CommandExt,
    process::{Command, Stdio},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runfiles = Runfiles::create()?;
    let fix = rlocation!(runfiles, env!("FIX_CLI")).ok_or("missing Fix CLI")?;
    let program = rlocation!(runfiles, env!("FIX_PROGRAM")).ok_or("missing program")?;
    let mut command = Command::new(&fix);
    command.env("RUNFILES_DIR", runfiles::find_runfiles_dir()?);
    command.env(
        "PATH",
        env::join_paths(
            std::iter::once(
                fix.parent()
                    .ok_or("missing command directory")?
                    .join("commands/bin"),
            )
            .chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
        )?,
    );
    let args = env::args_os().skip(1);
    if env!("FIX_MODE") == "flatware" {
        let output = Command::new(&fix)
            .args(["create-blob", "-"])
            .stdin(File::open(program)?)
            .output()?;
        if !output.status.success() {
            std::io::stderr().write_all(&output.stderr)?;
            std::process::exit(output.status.code().unwrap_or(1));
        }
        let handle = String::from_utf8(output.stdout)?;
        return Err(command
            .arg("flatware-run")
            .arg(handle.trim())
            .args(args)
            .exec()
            .into());
    }
    let mut source = format!("program = @\"{}\"\n*#(\"\" program", program.display());
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
