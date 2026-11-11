use fixcli::{
    Options, Shell,
    expressions::Expressions,
    shell::{Blob, Tree},
};
use runfiles::{Runfiles, rlocation};
use std::{
    env, fs, io,
    num::NonZeroUsize,
    path::PathBuf,
    process::{Command, ExitCode},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Repository(PathBuf);

impl Repository {
    fn temporary() -> io::Result<Self> {
        for index in 0u64.. {
            let path = env::temp_dir().join(format!("fix-run-{}-{index}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        unreachable!()
    }
}

impl Drop for Repository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn checked(command: &mut Command) -> Result<()> {
    let status = command.status()?;
    if !status.success() {
        return Err(format!("{} exited with {status}", command.get_program().display()).into());
    }
    Ok(())
}

fn run() -> Result<()> {
    let runfiles = Runfiles::create()?;
    let locate = |name| runfiles.rlocation(name).ok_or("missing launcher input");
    let fix = rlocation!(runfiles, env!("FIX_CLI")).ok_or("missing Fix CLI")?;
    let program = rlocation!(runfiles, env!("FIX_PROGRAM")).ok_or("missing program")?;
    let archive = locate(env!("FIX_ARCHIVE"))?;
    let repository = Repository::temporary()?;
    checked(Command::new(&fix).current_dir(&repository.0).arg("init"))?;
    checked(
        Command::new(&fix)
            .current_dir(&repository.0)
            .arg("import")
            .arg(archive),
    )?;

    let arguments = env::args_os()
        .skip(1)
        .map(|argument| {
            argument
                .into_string()
                .map_err(|_| "arguments must be Fix expressions")
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let jobs = match env::var("FIX_JOBS") {
        Ok(jobs) => jobs.parse::<NonZeroUsize>()?,
        Err(env::VarError::NotPresent) => std::thread::available_parallelism()?,
        Err(error) => return Err(error.into()),
    };
    let mut shell = Shell::open(
        &repository.0,
        Options {
            trust: Some(repository.0.join(".fix/trust")),
            jobs: jobs.get(),
            ..Options::default()
        },
    )?;
    {
        let module = shell.file(&program)?;
        let program = shell.compilation(&module, None)?;
        let mut fields = vec![Blob::new(b"").into_value(), program];
        fields.extend(Expressions::new(&mut shell, Vec::new()).arguments(&arguments)?);
        for module in env!("FIX_ARGUMENT_MODULES").lines() {
            let module = shell.file(&locate(module)?)?;
            fields.push(shell.compilation(&module, None)?);
        }
        let result = Tree::new(&fields).application().strict().eval()?;
        println!("{}", shell.save(&result)?);
    }
    shell.close()?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("fix-run: {error}");
            if let Some(fixcli::Error::Execution(code)) = error.downcast_ref::<fixcli::Error>() {
                ExitCode::from(*code)
            } else {
                ExitCode::from(2)
            }
        }
    }
}
