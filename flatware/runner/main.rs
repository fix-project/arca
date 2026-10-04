use anyhow::{Context, Result, bail, ensure};
use clap::{Parser, Subcommand};
use flatware_protocol::Object;
use std::{
    env,
    ffi::{OsStr, OsString},
    fs::{self, DirBuilder, File, FileTimes},
    io::{self, Write},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{DirBuilderExt, FileTypeExt, MetadataExt, symlink},
    },
    path::{Component, Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Parser)]
#[command(name = "flatware", about = "Run WASIp1 programs within Fix")]
struct Cli {
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    Run(Options),
}

#[derive(clap::Args)]
#[command(trailing_var_arg = true)]
struct Options {
    #[arg(long = "dir", value_name = "HOST[::GUEST]")]
    directories: Vec<OsString>,
    #[arg(long = "env", value_name = "NAME=VALUE")]
    environment: Vec<OsString>,
    #[arg(long, value_name = "FILE")]
    stdin: Option<PathBuf>,
    #[arg(long, hide = true)]
    native: bool,
    program: PathBuf,
    #[arg(allow_hyphen_values = true)]
    arguments: Vec<OsString>,
}

struct Mount {
    host: PathBuf,
    guest: OsString,
    destination: PathBuf,
}

impl Mount {
    fn parse(value: OsString) -> Result<Self> {
        let bytes = value.as_bytes();
        let (host, guest) = match bytes.windows(2).position(|pair| pair == b"::") {
            Some(index) => (&bytes[..index], &bytes[index + 2..]),
            None => (bytes, bytes),
        };
        ensure!(
            !host.is_empty() && !guest.is_empty(),
            "directory mount requires nonempty host and guest paths"
        );
        let host = PathBuf::from(OsStr::from_bytes(host))
            .canonicalize()
            .context("resolve directory mount")?;
        ensure!(host.is_dir(), "{} is not a directory", host.display());
        let guest = OsString::from_vec(guest.to_vec());
        let mut destination = PathBuf::new();
        for component in Path::new(&guest).components() {
            match component {
                Component::Normal(name) => destination.push(name),
                Component::CurDir | Component::RootDir => {}
                _ => bail!("guest directory paths cannot contain '..'"),
            }
        }
        Ok(Self {
            host,
            guest,
            destination,
        })
    }
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temporary_directory(prefix: &str) -> Result<PathBuf> {
    for index in 0u64.. {
        let path = env::temp_dir().join(format!("{prefix}-{}-{index}", std::process::id()));
        match DirBuilder::new().mode(0o700).create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    unreachable!()
}

fn tool(name: &str) -> Result<OsString> {
    env::var_os(format!("FLATWARE_{name}"))
        .with_context(|| format!("missing Flatware launcher resource {name}"))
}

fn execute(command: &mut Command) -> Result<()> {
    let output = command
        .stdin(Stdio::null())
        .output()
        .context("start Flatware tool")?;
    ensure!(
        output.status.success(),
        "{} failed: {}{}",
        command.get_program().to_string_lossy(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn compile(program: &Path, work: &Path) -> Result<PathBuf> {
    let wasm = work.join("module.wasm");
    let executable = work.join("program.elf");
    execute(
        Command::new(tool("COMPILE")?)
            .arg("merge")
            .arg(tool("WASM_MERGE")?)
            .arg(tool("CHECKER")?)
            .arg(tool("ADAPTER")?)
            .arg(tool("CONTROL")?)
            .arg(program)
            .arg(&wasm),
    )?;
    execute(
        Command::new(tool("COMPILE")?)
            .arg("compile")
            .arg(tool("GCC")?)
            .arg(tool("FLAGS")?)
            .arg(tool("MEMMAP")?)
            .arg(tool("SHELL")?)
            .arg(&executable)
            .arg(tool("WASM2C")?)
            .arg(&wasm)
            .arg(tool("WASM_RT")?)
            .arg(tool("HEADER")?),
    )?;
    Ok(executable)
}

fn timestamp(seconds: i64, nanos: i64) -> Result<u64> {
    ensure!(
        seconds >= 0 && (0..1_000_000_000).contains(&nanos),
        "file timestamp is outside the Flatware range"
    );
    (seconds as u64)
        .checked_mul(1_000_000_000)
        .and_then(|seconds| seconds.checked_add(nanos as u64))
        .context("file timestamp overflow")
}

fn snapshot(path: &Path, name: &[u8]) -> Result<Object> {
    let metadata =
        fs::symlink_metadata(path).with_context(|| format!("read {}", path.display()))?;
    let kind = metadata.file_type();
    let (filetype, contents) = if kind.is_dir() {
        let mut entries = fs::read_dir(path)?.collect::<io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        let entries = entries
            .into_iter()
            .map(|entry| snapshot(&entry.path(), entry.file_name().as_bytes()))
            .collect::<Result<Vec<_>>>()?;
        (3, Object::Tree(entries))
    } else if kind.is_file() {
        (4, Object::Blob(fs::read(path)?))
    } else if kind.is_symlink() {
        (7, Object::blob(fs::read_link(path)?.as_os_str().as_bytes()))
    } else {
        let description = if kind.is_fifo() {
            "FIFO"
        } else {
            "special file"
        };
        bail!("cannot snapshot {description} {}", path.display());
    };
    let size = match &contents {
        Object::Tree(children) => children.len(),
        Object::Blob(bytes) => bytes.len(),
    };
    let stat = Object::Tree(vec![
        Object::integer(0),
        Object::integer(metadata.ino()),
        Object::blob([filetype]),
        Object::integer(metadata.nlink()),
        Object::integer(size as u64),
        Object::integer(timestamp(metadata.atime(), metadata.atime_nsec())?),
        Object::integer(timestamp(metadata.mtime(), metadata.mtime_nsec())?),
        Object::integer(timestamp(metadata.ctime(), metadata.ctime_nsec())?),
    ]);
    Ok(Object::Tree(vec![Object::blob(name), stat, contents]))
}

fn stream(name: &str, bytes: Vec<u8>) -> Object {
    Object::Tree(vec![
        Object::blob(name),
        Object::Tree(vec![]),
        Object::Blob(bytes),
    ])
}

fn tree(object: &Object) -> Result<&[Object]> {
    object
        .tree()
        .context("expected a Tree in the Flatware result")
}
fn bytes(object: &Object) -> Result<&[u8]> {
    object
        .bytes()
        .context("expected a Blob in the Flatware result")
}
fn fields<const N: usize>(object: &Object) -> Result<&[Object; N]> {
    tree(object)?
        .try_into()
        .context("invalid Flatware field count")
}
fn integer(object: &Object) -> Result<u64> {
    Ok(u64::from_le_bytes(
        bytes(object)?
            .try_into()
            .context("expected an eight-byte integer")?,
    ))
}

fn materialize(descriptor: &Object, path: &Path) -> Result<()> {
    let [_, metadata, contents] = fields(descriptor)?;
    let [dev, _, filetype, _, _, atim, mtim, _] = fields(metadata)?;
    ensure!(integer(dev)? == 0, "unsupported Flatware filesystem device");
    match bytes(filetype)? {
        [3] => {
            fs::create_dir_all(path)?;
            ensure!(
                fs::symlink_metadata(path)?.is_dir(),
                "expected output directory"
            );
            for child in tree(contents)? {
                let [name, _, _] = fields(child)?;
                let name = bytes(name)?;
                ensure!(
                    !name.is_empty()
                        && name != b"."
                        && name != b".."
                        && !name.contains(&b'/')
                        && !name.contains(&0),
                    "invalid Flatware directory entry name"
                );
                materialize(child, &path.join(OsStr::from_bytes(name)))?;
            }
        }
        [4] => {
            let mut file = File::create_new(path)?;
            file.write_all(bytes(contents)?)?;
        }
        [7] => {
            symlink(OsStr::from_bytes(bytes(contents)?), path)?;
            return Ok(());
        }
        _ => bail!("unsupported Flatware output file type"),
    }
    File::open(path)?.set_times(
        FileTimes::new()
            .set_accessed(UNIX_EPOCH + Duration::from_nanos(integer(atim)?))
            .set_modified(UNIX_EPOCH + Duration::from_nanos(integer(mtim)?)),
    )?;
    Ok(())
}

fn run(options: Options) -> Result<u8> {
    let mounts = options
        .directories
        .into_iter()
        .map(Mount::parse)
        .collect::<Result<Vec<_>>>()?;
    for (index, mount) in mounts.iter().enumerate() {
        for previous in &mounts[..index] {
            ensure!(
                !mount.destination.starts_with(&previous.destination)
                    && !previous.destination.starts_with(&mount.destination),
                "guest directory mounts must not overlap"
            );
        }
    }
    let work = Scratch(temporary_directory("flatware-work")?);
    let program = if options.native {
        options.program.clone()
    } else {
        compile(&options.program, &work.0)?
    };
    let mut arguments = vec![Object::blob(
        options
            .program
            .file_stem()
            .context("program name")?
            .as_bytes(),
    )];
    arguments.extend(
        options
            .arguments
            .iter()
            .map(|argument| Object::blob(argument.as_bytes())),
    );
    let environment = options
        .environment
        .iter()
        .map(|value| {
            let value = value.as_bytes();
            ensure!(
                value
                    .iter()
                    .position(|&byte| byte == b'=')
                    .is_some_and(|index| index > 0),
                "environment entries must be NAME=VALUE"
            );
            Ok(Object::blob(value))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut descriptors = vec![
        stream(
            "stdin",
            options.stdin.map(fs::read).transpose()?.unwrap_or_default(),
        ),
        stream("stdout", vec![]),
        stream("stderr", vec![]),
    ];
    descriptors.extend(
        mounts
            .iter()
            .map(|mount| snapshot(&mount.host, mount.guest.as_bytes()))
            .collect::<Result<Vec<_>>>()?,
    );
    let time: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_nanos()
        .try_into()
        .context("starting time overflow")?;
    let input = Object::Tree(vec![
        Object::Blob(fs::read(program)?),
        Object::Tree(arguments),
        Object::Tree(environment),
        Object::Tree(descriptors),
        Object::integer(0),
        Object::integer(time),
    ]);
    let input_path = work.0.join("input");
    let output_path = work.0.join("output");
    fs::write(&input_path, postcard::to_allocvec(&input)?)?;
    execute(
        Command::new(tool("VMM")?)
            .arg(tool("KERNEL")?)
            .args(["--smp", "2"])
            .arg(&input_path)
            .arg(&output_path),
    )?;
    let output = fs::read(output_path)?;
    let output: Object = postcard::from_bytes(&output)?;
    let [descriptors, status] = fields(&output)?;
    let descriptors = tree(descriptors)?;
    ensure!(
        descriptors.len() == mounts.len() + 3,
        "invalid Flatware descriptor count"
    );
    let status = u32::from_le_bytes(
        bytes(status)?
            .try_into()
            .context("expected four-byte exit status")?,
    );
    let directory = temporary_directory("flatware-output")?;
    for (mount, descriptor) in mounts.iter().zip(&descriptors[3..]) {
        let [name, _, _] = fields(descriptor)?;
        ensure!(
            bytes(name)? == mount.guest.as_bytes(),
            "Flatware changed a preopen name"
        );
        materialize(descriptor, &directory.join(&mount.destination))
            .with_context(|| format!("write output directory {}", directory.display()))?;
    }
    let [_, _, stdout] = fields(&descriptors[1])?;
    io::stdout().lock().write_all(bytes(stdout)?)?;
    let mut stderr = io::stderr().lock();
    let [_, _, diagnostics] = fields(&descriptors[2])?;
    let diagnostics = bytes(diagnostics)?;
    stderr.write_all(diagnostics)?;
    if !diagnostics.is_empty() && !diagnostics.ends_with(b"\n") {
        stderr.write_all(b"\n")?;
    }
    writeln!(stderr, "output directory: {}", directory.display())?;
    Ok(status as u8)
}

fn main() -> ExitCode {
    let Cli {
        command: Action::Run(options),
    } = Cli::parse();
    match run(options) {
        Ok(status) => ExitCode::from(status),
        Err(error) => {
            eprintln!("flatware: {error:#}");
            ExitCode::FAILURE
        }
    }
}
