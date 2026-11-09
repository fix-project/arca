use runfiles::Runfiles;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let files = Runfiles::create().unwrap();
    let directory = runfiles::find_runfiles_dir().ok();
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    for path in env!("BENCHMARK_SUITES").lines() {
        let executable = files.rlocation(path).unwrap();
        let mut command = Command::new(executable);
        command.args(&args);
        if let Some(directory) = &directory {
            command.env("RUNFILES_DIR", directory);
        }
        let status = command.status().unwrap();
        if !status.success() {
            return ExitCode::from(u8::try_from(status.code().unwrap_or(1)).unwrap_or(1));
        }
    }
    ExitCode::SUCCESS
}
