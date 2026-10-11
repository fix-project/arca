use clap::{Arg, ArgAction, Command as CliCommand};
use std::{
    env,
    ffi::OsString,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::PathBuf,
    process::{Command, ExitCode},
};

fn memory_size(value: &str) -> Result<usize, String> {
    let mib: usize = value.parse().map_err(|_| "memory must be an integer")?;
    if mib < 16 || !mib.is_power_of_two() {
        return Err("memory must be a power of two, at least 16 MiB".into());
    }
    mib.checked_mul(1 << 20)
        .ok_or_else(|| "memory size overflow".into())
}

fn cli() -> CliCommand {
    let mut cli = CliCommand::new("fix")
        .about("Create, inspect, and evaluate Fix values")
        .arg(Arg::new("directory").short('C').value_name("DIR").value_parser(clap::value_parser!(PathBuf))
            .action(ArgAction::Append).help("Change directory before running the command"))
        .allow_external_subcommands(true)
        .after_help("External commands are found in .fix/commands and PATH.\nBundled commands: parse, run, flatware-application, flatware-run, flatware-link, compile, json-import, import, export, flatware-import, flatware-export.");
    for (name, about) in [
        ("init", "Initialize a Fix repository"),
        (
            "repository",
            "Print the absolute path to the active .fix directory",
        ),
        ("eval", "Evaluate a value and store its result"),
        ("think", "Perform one thunk reduction and store its result"),
        (
            "visit",
            "Print reachable stored Objects without evaluating thunks",
        ),
        (
            "create-blob",
            "Create a Blob from a file, stdin, text, or an integer",
        ),
        ("create-tree", "Create a Tree from values"),
        ("read-blob", "Write a Blob to standard output"),
        ("read-tree", "Print a Tree's children"),
        ("create-ref", "Create an object reference"),
        (
            "create-identification-thunk",
            "Create an identification thunk",
        ),
        ("create-application-thunk", "Create an application thunk"),
        ("create-selection-thunk", "Create a selection thunk"),
        ("create-strict-encode", "Create a strict encode"),
        ("create-shallow-encode", "Create a shallow encode"),
        ("label", "List, read, or assign labels"),
        ("resolve", "Resolve a value or label"),
        ("get-type", "Print a value's Fix type"),
        ("get-data-type", "Print an object's data type"),
        ("get-encode-type", "Print an encode's type"),
        ("len", "Print a value's length"),
        ("is-tag", "Test whether a value is a tag"),
        ("is-eq", "Test whether a value has Eq semantics"),
    ] {
        let mut command = CliCommand::new(name).about(about);
        if !matches!(name, "init" | "repository") {
            command = command.arg_required_else_help(true);
        }
        if name.starts_with("create-") {
            command = command.arg(
                Arg::new("label")
                    .short('l')
                    .long("label")
                    .value_name("NAME"),
            );
        }
        command = match name {
            "init" | "repository" => command,
            "create-blob" => command.arg(
                Arg::new("values")
                    .value_name("FILE")
                    .num_args(1..)
                    .required_unless_present_any(["format", "files-from"])
                    .conflicts_with_all(["format", "files-from"])
                    .help("Input file; use - for stdin"),
            ),
            "label" => command
                .subcommand_required(true)
                .subcommand(CliCommand::new("list").about("List defined labels"))
                .subcommand(
                    CliCommand::new("get")
                        .about("Read a label")
                        .arg(Arg::new("name").required(true)),
                )
                .subcommand(
                    CliCommand::new("set")
                        .about("Assign a label to a value")
                        .arg(Arg::new("name").required(true))
                        .arg(Arg::new("value").required(true)),
                ),
            "create-tree" => command.arg(
                Arg::new("values")
                    .num_args(1..)
                    .required(true)
                    .help("Values or labels; use - to read children from stdin"),
            ),
            _ => command.arg(
                Arg::new("values")
                    .required(true)
                    .help("Value or label; use - for stdin"),
            ),
        };
        if matches!(name, "create-blob" | "read-blob") {
            for format in ["string", "u8", "u32", "u64"] {
                let mut argument = Arg::new(format).long(format).group("format");
                argument = if name == "create-blob" {
                    argument.value_name("VALUE").allow_hyphen_values(true)
                } else {
                    argument.action(ArgAction::SetTrue)
                };
                command = command.arg(argument);
            }
        }
        if name == "create-blob" {
            command = command.arg(
                Arg::new("files-from")
                    .long("files-from")
                    .value_name("MANIFEST")
                    .conflicts_with("format")
                    .help("Read NUL-separated filenames; use - for stdin"),
            );
        }
        if name == "create-tree" {
            command = command.arg(
                Arg::new("bindings")
                    .long("bindings")
                    .action(ArgAction::SetTrue)
                    .help("Read NUL-separated name/value pairs from stdin"),
            );
        }
        if name == "read-tree" {
            command = command.arg(
                Arg::new("labels")
                    .long("labels")
                    .action(ArgAction::SetTrue)
                    .help("Print matching labels beside each child handle"),
            );
        }
        if name == "eval" {
            command = command
                .arg(
                    Arg::new("memory")
                        .long("memory")
                        .value_name("MiB")
                        .value_parser(memory_size)
                        .default_value("16384")
                        .help("VM memory budget (power of two, in MiB)"),
                )
                .arg(
                    Arg::new("jobs")
                        .short('j')
                        .long("jobs")
                        .env("FIX_JOBS")
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize))
                        .help("Evaluation parallelism (default: available CPUs)"),
                );
        }
        cli = cli.subcommand(command);
    }
    cli
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let matches = cli().get_matches();
    if let Some(directories) = matches.get_many::<PathBuf>("directory") {
        for directory in directories {
            env::set_current_dir(directory)?;
        }
    }
    let Some((command, options)) = matches.subcommand() else {
        cli().print_help()?;
        println!();
        return Ok(());
    };
    let available = std::thread::available_parallelism()?.get();
    let builtin = cli().find_subcommand(command).is_some();
    if builtin {
        let jobs = if command == "eval" {
            *options.get_one::<usize>("jobs").unwrap_or(&available)
        } else {
            1
        };
        if jobs == 0 {
            return Err("jobs must be a positive integer".into());
        }
        let memory = if command == "eval" {
            *options.get_one::<usize>("memory").unwrap()
        } else {
            1 << 34
        };
        let mut args = vec![OsString::from(command)];
        if command == "create-tree" && options.get_flag("bindings") {
            args.push("--bindings".into());
        }
        if command == "create-blob"
            && let Some(path) = options.get_one::<String>("files-from")
        {
            args.extend(["--files-from".into(), path.into()]);
        }
        if command == "read-tree" && options.get_flag("labels") {
            args.push("--labels".into());
        }
        if command == "label" {
            let (action, options) = options.subcommand().unwrap();
            args.push(action.into());
            if matches!(action, "get" | "set") {
                args.push(options.get_one::<String>("name").unwrap().into());
            }
            if action == "set" {
                args.push(options.get_one::<String>("value").unwrap().into());
            }
        }
        if matches!(command, "create-blob" | "read-blob") {
            for format in ["string", "u8", "u32", "u64"] {
                if command == "create-blob" {
                    if let Some(value) = options.get_one::<String>(format) {
                        args.extend([format!("--{format}").into(), value.into()]);
                    }
                } else if options.get_flag(format) {
                    args.push(format!("--{format}").into());
                }
            }
        }
        if command != "init"
            && command != "repository"
            && command != "label"
            && let Some(values) = options.get_many::<String>("values")
        {
            args.extend(values.map(OsString::from));
        }
        if command.starts_with("create-")
            && let Some(label) = options.get_one::<String>("label")
        {
            args.extend([OsString::from("-l"), label.into()]);
        }
        let mut argv = vec![String::from("fix")];
        argv.extend(
            args.into_iter()
                .map(OsString::into_string)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "arguments must be UTF-8")?,
        );
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
        let kernel: &[u8] = include_bytes!(env!("FIX_KERNEL"));
        vmm::runtime::Runtime::new(jobs, memory, kernel.into()).run(argv);
        Ok(())
    } else {
        if command.contains('/') || command == "." || command == ".." {
            return Err("invalid command name".into());
        }
        let name = format!("fix-{command}");
        let local = env::current_dir()?.ancestors().find_map(|dir| {
            let repo = dir.join(".fix");
            repo.exists().then(|| repo.join("commands").join(&name))
        });
        let path = local
            .filter(|path| path.is_file())
            .or_else(|| {
                env::split_paths(&env::var_os("PATH").unwrap_or_default())
                    .map(|dir| dir.join(&name))
                    .find(|path| {
                        path.metadata().is_ok_and(|metadata| {
                            metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                        })
                    })
            })
            .ok_or_else(|| format!("unknown command {command:?}; see 'fix --help'"))?;
        let mut child = Command::new(path);
        child.args(options.get_many::<OsString>("").into_iter().flatten());
        child.env("FIX", env::current_exe()?);
        if env::var_os("FIX_JOBS").is_none() {
            child.env("FIX_JOBS", available.to_string());
        }
        Err(child.exec().into())
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("fix: {error}");
            ExitCode::from(2)
        }
    }
}
