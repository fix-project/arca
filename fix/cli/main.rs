mod repl;

use clap::{Args, CommandFactory, Parser, Subcommand};
use fix::{FailureKind, Thunk};
use fixcli::{
    Memoization, Options, Shell as Client, commands,
    configuration::{self, ConfigCommand, RemoteCommand, TrustCommand},
    expressions::{Expressions, Value},
    shell::{Blob, Tree},
};
use std::{
    env,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Evaluates Fix expressions and manages Fix repositories.
///
/// With no command, parses EXPRESSION, evaluates its imports, and prints the
/// resulting handle. Expression imports are @handle, @label, @file, @compile,
/// @script and @todo.
#[derive(Parser)]
#[command(name = "fix", subcommand_value_name = "COMMAND")]
struct Cli {
    #[command(flatten)]
    global: Global,
    #[command(subcommand)]
    command: Option<Command>,
    /// A Fix expression.
    expression: Option<String>,
}

#[derive(Args)]
#[command(next_help_heading = "Global options")]
struct Global {
    /// Change to DIR first (repeatable).
    #[arg(short = 'C', value_name = "DIR", global = true)]
    directories: Vec<PathBuf>,
    /// Label the result.
    #[arg(short, long, value_name = "NAME", global = true)]
    label: Option<String>,
    /// Bind NAME to the value of EXPRESSION (repeatable).
    #[arg(short, long = "bind", value_name = "NAME=EXPRESSION", value_parser = binding, global = true)]
    bindings: Vec<(String, String)>,
    /// Application workers; 0 disables local applications [default: all CPUs]
    #[arg(short, long, value_name = "N", env = "FIX_JOBS", global = true)]
    jobs: Option<usize>,
    /// Write a time-weighted guest call graph in Linux perf.data format.
    #[arg(long, value_name = "PATH", global = true)]
    perf: Option<PathBuf>,
    /// VM memory budget in MiB, a power of two.
    #[arg(long, value_name = "MIB", global = true)]
    memory: Option<usize>,
    /// Persist coupons: auto, always, or never.
    #[arg(long, value_name = "MODE", default_value_t = Memoization::Auto, global = true)]
    memoize: Memoization,
    /// SSH session creation ID (32 hexadecimal digits).
    #[arg(long, global = true)]
    session_id: Option<String>,
    /// Join the existing repository session and use its settings.
    #[arg(long, global = true)]
    join: bool,
    /// Scheduler Runnable.
    #[arg(long, value_name = "NAME", env = "FIX_SCHEDULER", global = true)]
    scheduler: Option<String>,
    /// Coupon collector.
    #[arg(long, value_name = "NAME", env = "FIX_COLLECTOR", global = true)]
    collector: Option<String>,
    /// Parser Runnable.
    #[arg(long, value_name = "NAME", global = true)]
    parser: Option<String>,
    /// Compiler Runnable.
    #[arg(long, value_name = "NAME", global = true)]
    compiler: Option<String>,
    /// Add or override a remote for this command (repeatable).
    #[arg(long = "remote", value_name = "NAME=URL", value_parser = configuration::remote_argument, global = true)]
    remotes: Vec<fixrepository::Remote>,
    /// Select a Fix scheduler name for one remote during this command.
    #[arg(long = "remote-scheduler", value_name = "REMOTE=FIX_NAME", value_parser = configuration::remote_name_argument, global = true)]
    remote_schedulers: Vec<(String, String)>,
    /// Select a Fix collector name for one remote during this command.
    #[arg(long = "remote-collector", value_name = "REMOTE=FIX_NAME", value_parser = configuration::remote_name_argument, global = true)]
    remote_collectors: Vec<(String, String)>,
    /// Override signing identities with public keys or public-key files (repeatable).
    #[arg(long = "identity", value_name = "KEY", global = true)]
    identities: Vec<String>,
    /// Use an unsigned staging repository for bootstrap builds.
    #[arg(long, env = "FIX_UNSIGNED", global = true)]
    unsigned: bool,
    /// Report evaluation timings on stderr.
    #[arg(long, global = true)]
    profile: bool,
    /// Print full Fix handles instead of friendly display names.
    #[arg(long, global = true)]
    no_abbrev: bool,
    /// Print a per-machine resource summary after evaluation.
    #[arg(long, global = true)]
    summarize: bool,
}

#[derive(Args)]
#[group(multiple = false)]
struct TransferMode {
    /// Evaluate a shallow Encode and store the resulting Ref (default).
    #[arg(long)]
    shallow: bool,
    /// Evaluate a strict Encode and store the resulting Object.
    #[arg(long)]
    strict: bool,
    /// Preserve the value and load its backing without evaluating it.
    #[arg(long)]
    exact: bool,
}
impl TransferMode {
    fn resolve(&self, operation: &str) -> Result<fixcli::remotes::Mode> {
        use fixcli::remotes::Mode;
        let selected = if self.shallow {
            Some(Mode::Shallow)
        } else if self.strict {
            Some(Mode::Strict)
        } else if self.exact {
            Some(Mode::Exact)
        } else {
            None
        };
        Mode::configured(operation, selected)
    }
}

#[derive(Subcommand)]
enum Command {
    #[command(hide = true)]
    Worker { lock_fd: i32, listen_fd: i32 },

    /// Create an empty repository in the current directory.
    Init,
    /// Install a bootstrap seed and default programs in the user Fix store.
    Bootstrap { seed: PathBuf },
    /// Serve remote evaluation requests from this repository over TCP.
    Serve {
        /// TCP address to bind (for example, 127.0.0.1:9000).
        #[arg(long, value_name = "HOST:PORT", required_unless_present = "stdio")]
        listen: Option<String>,
        /// Speak the Fix protocol over standard input and output (for SSH).
        #[arg(long, conflicts_with = "listen")]
        stdio: bool,
    },
    /// Read and evaluate low-level Fix expressions interactively.
    Repl,
    #[command(subcommand)]
    Remote(RemoteCommand),
    /// Read or edit user or repository settings.
    Config {
        /// Use user configuration rather than this repository's configuration.
        #[arg(long, conflicts_with = "local")]
        global: bool,
        /// Use this repository's configuration.
        #[arg(long)]
        local: bool,
        #[arg(long, value_name = "KEY", conflicts_with_all = ["unset", "list"])]
        get: Option<String>,
        #[arg(long, value_name = "KEY", conflicts_with_all = ["get", "list"])]
        unset: Option<String>,
        #[arg(long, conflicts_with_all = ["get", "unset"])]
        list: bool,
        /// KEY [VALUE]: read or set a setting.
        #[arg(num_args = 0..=2)]
        arguments: Vec<String>,
    },
    /// Create an empty repository with `origin` and fetch its refs.
    Clone { address: String, directory: PathBuf },
    /// Fetch and realize remote labels (all remotes by default).
    Pull {
        remote: Option<String>,
        #[arg(requires = "remote")]
        source: Option<String>,
        #[arg(requires = "source")]
        destination: Option<String>,
        #[command(flatten)]
        mode: TransferMode,
    },
    /// Realize local labels and publish them to an SSH or file remote.
    Push {
        remote: String,
        source: Option<String>,
        #[arg(requires = "source")]
        destination: Option<String>,
        #[command(flatten)]
        mode: TransferMode,
    },
    #[command(subcommand)]
    Trust(TrustCommand),
    /// Evaluate an expression.
    Eval {
        expression: String,
        /// Wrap the input Thunk in a strict Encode before evaluating it.
        #[arg(long)]
        encode: bool,
    },
    /// Take one Think step.
    Think { expression: String },
    /// Compile a Wasm module with `compile`.
    Compile {
        expression: String,
        /// Number of C translation units [default: one per 128 KiB of Wasm]
        units: Option<u64>,
        /// Memory budget for the compile wrapper, in MiB [default: scales with unit count]
        #[arg(long, value_name = "MIB", value_parser = clap::value_parser!(u64).range(1..=(u64::MAX >> 20)))]
        compile_memory: Option<u64>,
        /// Memory budget per wasm2c invocation, in MiB [default: scales with module size]
        #[arg(long, value_name = "MIB", value_parser = clap::value_parser!(u64).range(1..=(u64::MAX >> 20)))]
        wasm2c_memory: Option<u64>,
        /// Memory budget per Clang invocation, including linking, in MiB
        #[arg(long, value_name = "MIB", value_parser = clap::value_parser!(u64).range(1..=(u64::MAX >> 20)))]
        clang_memory: Option<u64>,
    },
    /// Record or list where Todo values can be fetched.
    #[command(subcommand)]
    Todo(Todo),
    /// Parse a script (`-` for stdin) without evaluating it.
    Parse { file: String },
    /// Evaluate a Fix script or its shebang expression (`-` for stdin).
    Run {
        /// Value of $limit in the shebang. K, M, and G use powers of 1024 [default: 512M].
        #[arg(long, value_name = "SIZE", value_parser = application_limit)]
        limit: Option<u64>,
        file: String,
        /// Positional arguments as Fix expressions.
        #[arg(value_name = "ARG")]
        arguments: Vec<String>,
    },
    /// Write the objects reachable from expressions to an archive.
    Export {
        /// Export EXPRESSION's handle without its data.
        #[arg(long = "without-backing", value_name = "EXPRESSION")]
        without_backing: Vec<String>,
        expression: String,
        archive: PathBuf,
        /// More values to export.
        expressions: Vec<String>,
    },
    /// Write a Flatware tree to PATH.
    FlatwareExport { expression: String, path: PathBuf },
    #[command(flatten)]
    Plumbing(commands::Plumbing),
}

#[derive(Subcommand)]
enum Todo {
    /// `add URI` imports a Todo for URI; `add EXPRESSION URI...` records URIs for a value.
    Add {
        #[arg(required = true, value_name = "ARGUMENT")]
        arguments: Vec<String>,
    },
    /// Print the URIs recorded for a value.
    List { expression: String },
}

fn binding(argument: &str) -> std::result::Result<(String, String), String> {
    let (name, expression) = argument.split_once('=').ok_or("expected NAME=EXPRESSION")?;
    let mut characters = name.chars();
    let valid = characters
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && characters.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
    if !valid {
        return Err(format!("invalid binding name {name:?}"));
    }
    Ok((name.into(), expression.into()))
}

fn is_command(argument: &str) -> bool {
    argument.starts_with(|ch: char| ch.is_ascii_lowercase())
        && argument
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

fn hex_session_id(text: &str, id: &mut [u8; 16]) -> Result<()> {
    if text.len() != 32 {
        return Err("session ID must contain 32 hexadecimal digits".into());
    }
    for (byte, pair) in id.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair)?, 16)?;
    }
    Ok(())
}

impl Global {
    fn options(&self) -> Result<Options> {
        let settings = configuration::settings()?;
        Ok(Options {
            executable: env::current_exe()?,
            scheduler: self
                .scheduler
                .clone()
                .or_else(|| settings.get("scheduler").cloned())
                .unwrap_or_else(|| "scheduler".into()),
            collector: self
                .collector
                .clone()
                .or_else(|| settings.get("collector").cloned())
                .unwrap_or_else(|| "collector".into()),
            parser: self
                .parser
                .clone()
                .or_else(|| settings.get("parser").cloned())
                .unwrap_or_else(|| "fix-parser".into()),
            compiler: self
                .compiler
                .clone()
                .or_else(|| settings.get("compiler").cloned())
                .unwrap_or_else(|| "compile".into()),
            remotes: self.remotes.clone(),
            remote_schedulers: self.remote_schedulers.clone(),
            remote_collectors: self.remote_collectors.clone(),
            identities: self
                .identities
                .iter()
                .map(|identity| fixcertificates::linux::key(identity))
                .collect::<std::result::Result<_, _>>()?,
            unsigned: self.unsigned,
            jobs: match self.jobs {
                Some(jobs) => jobs,
                None => {
                    configuration::jobs(settings.get("jobs").map(String::as_str).unwrap_or("auto"))?
                }
            },
            memory_mib: match self.memory {
                Some(memory) => configuration::memory(&memory.to_string())?,
                None => configuration::memory(
                    settings
                        .get("memory")
                        .map(String::as_str)
                        .unwrap_or("16384"),
                )?,
            },
            memoize: self.memoize,
            session_id: self
                .session_id
                .as_ref()
                .map(|text| {
                    let mut id = [0; 16];
                    hex_session_id(text, &mut id)?;
                    Ok::<_, Box<dyn std::error::Error>>(id)
                })
                .transpose()?,
            join_session: self.join,
            ..Options::default()
        })
    }

    fn open(&self, options: Options) -> Result<Client> {
        let mut client = Client::open(Path::new("."), options)?;
        client.set_profile(self.profile);
        Ok(client)
    }

    fn finish(&self, client: &mut Client, value: &Value) -> Result<u8> {
        if let Some(label) = &self.label {
            client.label(label, value)?;
        }
        let name = client.save(value)?;
        let mut stdout = std::io::stdout().lock();
        writeln!(stdout, "{name}")?;
        stdout.flush()?;
        Ok(0)
    }
}

/// Converts expression operands to persisted handle names.
fn handles(expressions: &mut Expressions, sources: &[String]) -> Result<Vec<String>> {
    if sources.is_empty() {
        return Ok(Vec::new());
    }
    let values = expressions.arguments(sources)?;
    values
        .iter()
        .map(|value| Ok(expressions.client.save(value)?))
        .collect()
}

fn one(expressions: &mut Expressions, source: &str) -> Result<Value> {
    Ok(expressions.arguments(&[source.to_owned()])?.remove(0))
}

fn application_limit(text: &str) -> std::result::Result<u64, &'static str> {
    let shift = match text.as_bytes().last() {
        Some(b'K' | b'k') => 10,
        Some(b'M' | b'm') => 20,
        Some(b'G' | b'g') => 30,
        _ => 0,
    };
    let digits = if shift == 0 {
        text
    } else {
        &text[..text.len() - 1]
    };
    digits
        .parse::<u64>()
        .ok()
        .and_then(|value| value.checked_mul(1 << shift))
        .filter(|value| *value > 0)
        .ok_or("expected a positive byte count, optionally followed by K, M, or G")
}

fn result_pair(value: &Value) -> Option<(Value, Value)> {
    let tree = Tree::try_from(value).ok()?;
    if tree.len() != 2 || tree.is_tag() {
        return None;
    }
    Some((tree.get(0).ok()??, tree.get(1).ok()??))
}

fn check_continuation(value: &Value) -> Option<()> {
    let tree = Tree::try_from(value).ok()?;
    if !tree.is_tag() || tree.len() != 3 {
        return None;
    }
    let author = Tree::try_from(tree.get(0).ok()??).ok()?;
    if author.is_tag() || !author.is_empty() {
        return None;
    }
    let marker = tree.get(2).ok()??;
    marker
        .equals(&Blob::new(b"Continuation").into_value())
        .ok()?
        .then_some(())
}

fn result_diagnostic(value: &Value) -> Option<String> {
    let (first, second) = result_pair(value)?;
    let (marker, detail, effect) = if let Some((marker, detail)) = result_pair(&first) {
        check_continuation(&second)?;
        (marker, detail, true)
    } else {
        (first, second, false)
    };
    let marker = Blob::try_from(marker).ok()?;
    let mut bytes = [0; 16];
    let marker_bytes = bytes.get_mut(..marker.len())?;
    marker.read_exact(0, marker_bytes).ok()?;
    let kind = FailureKind::from_marker(marker_bytes)?;
    if effect && !matches!(kind, FailureKind::Error) {
        return None;
    }
    let detail = Blob::try_from(detail).ok()?.to_vec().ok()?;
    kind.decode(&detail).map(|failure| failure.to_string())
}

fn run() -> Result<u8> {
    let Cli {
        global,
        command,
        expression,
    } = Cli::parse();
    if let Some(Command::Worker { lock_fd, listen_fd }) = command {
        let kernel = std::sync::Arc::from(&include_bytes!(env!("FIX_KERNEL"))[..]);
        return Ok(unsafe { vmm::service::run_owner(kernel, lock_fd, listen_fd) }?);
    }
    common::progress::summarize(global.summarize);
    common::progress::abbreviate(!global.no_abbrev);
    for directory in &global.directories {
        env::set_current_dir(directory)?;
    }
    if global.unsigned {
        unsafe {
            env::set_var("FIX_UNSIGNED", "true");
        }
    }
    let perf = global
        .perf
        .as_ref()
        .map(fixcli::PerfCapture::start)
        .transpose()?;
    let evaluation = matches!(
        &command,
        Some(
            Command::Eval { .. }
                | Command::Think { .. }
                | Command::Compile { .. }
                | Command::Run { .. }
        )
    ) || command.is_none() && expression.is_some();
    let progress = evaluation
        .then(|| common::progress::Progress::advised(global.label.as_deref().unwrap_or_default()));
    let result = run_command(global, command, expression);
    if let Some(progress) = progress
        && matches!(result, Ok(0))
    {
        progress.finish();
    }
    if let Some(perf) = perf {
        perf.finish()?;
    }
    result
}

fn run_command(global: Global, command: Option<Command>, expression: Option<String>) -> Result<u8> {
    let options = if matches!(&command, Some(Command::Config { .. })) {
        Options::default()
    } else {
        global.options()?
    };
    let bindings = global.bindings.clone();
    let Some(command) = command else {
        let Some(expression) = expression else {
            Cli::command().print_help()?;
            return Ok(0);
        };
        if is_command(&expression) {
            return Err(format!("unknown command {expression:?}; see 'fix --help'").into());
        }
        let mut client = global.open(options)?;
        let mut expressions = Expressions::new(&mut client, bindings);
        let parsed = expressions.parse(&expression, &env::current_dir()?)?;
        let value = expressions.realize(parsed)?;
        return global.finish(&mut client, &value);
    };
    if !bindings.is_empty()
        && matches!(
            command,
            Command::Init
                | Command::Bootstrap { .. }
                | Command::Plumbing(_)
                | Command::Remote(_)
                | Command::Config { .. }
                | Command::Clone { .. }
                | Command::Pull { .. }
                | Command::Push { .. }
                | Command::Trust(_)
                | Command::Serve { .. }
        )
    {
        return Err("this command takes no -b bindings".into());
    }
    if global.label.is_some()
        && matches!(
            command,
            Command::Init
                | Command::Bootstrap { .. }
                | Command::Remote(_)
                | Command::Config { .. }
                | Command::Clone { .. }
                | Command::Pull { .. }
                | Command::Push { .. }
                | Command::Trust(_)
                | Command::Todo(_)
                | Command::Export { .. }
                | Command::FlatwareExport { .. }
                | Command::Repl
                | Command::Serve { .. }
        )
    {
        return Err("-l labels an expression result".into());
    }
    match command {
        Command::Worker { .. } => unreachable!(),
        Command::Serve { listen, stdio } => {
            global.open(options)?.serve(listen.as_deref(), stdio)?;
            Ok(0)
        }
        Command::Repl => repl::run(global.open(options)?, bindings),
        Command::Eval { ref expression, .. }
        | Command::Think { ref expression }
        | Command::Compile { ref expression, .. } => {
            let mut client = global.open(options)?;
            let mut expressions = Expressions::new(&mut client, bindings);
            let input = one(&mut expressions, expression)?;
            let value = match command {
                Command::Eval { encode, .. } => {
                    let input = if encode {
                        Thunk::try_from(input)
                            .map_err(fixcli::Error::from)?
                            .strict()
                            .into_value()
                    } else {
                        input
                    };
                    input.eval()?
                }
                Command::Think { .. } => client.think(&input)?,
                Command::Compile {
                    units,
                    compile_memory,
                    wasm2c_memory,
                    clang_memory,
                    ..
                } => {
                    let input = if input.value_type() == fix::ValueType::Thunk {
                        Thunk::try_from(input)
                            .map_err(fixcli::Error::from)?
                            .strict()
                            .into_value()
                    } else {
                        input
                    };
                    client.compilation(
                        &input,
                        fixcli::CompilationOptions {
                            units,
                            memory: compile_memory.map(|mib| mib << 20),
                            wasm2c_memory: wasm2c_memory.map(|mib| mib << 20),
                            clang_memory: clang_memory.map(|mib| mib << 20),
                        },
                    )?
                }
                _ => unreachable!(),
            };
            global.finish(&mut client, &value)
        }
        Command::Todo(todo) => {
            let mut client = global.open(options)?;
            match todo {
                Todo::Add { arguments } => match arguments.as_slice() {
                    [uri] => {
                        let value = client.todo_import(uri)?;
                        global.finish(&mut client, &value)
                    }
                    [source, uris @ ..] => {
                        let mut expressions = Expressions::new(&mut client, bindings);
                        let value = one(&mut expressions, source)?;
                        client.todo_add(&value, uris)?;
                        Ok(0)
                    }
                    [] => unreachable!(),
                },
                Todo::List { expression } => {
                    let mut expressions = Expressions::new(&mut client, bindings);
                    let value = one(&mut expressions, &expression)?;
                    let mut stdout = std::io::stdout().lock();
                    for uri in client.todo_list(&value)? {
                        stdout.write_all(&uri)?;
                        stdout.write_all(b"\n")?;
                    }
                    Ok(0)
                }
            }
        }
        Command::Parse { file } => {
            let mut client = global.open(options)?;
            let mut expressions = Expressions::new(&mut client, bindings);
            let value = if file == "-" {
                let source = std::io::read_to_string(std::io::stdin())?;
                expressions.parse(&source, &env::current_dir()?)?
            } else {
                expressions.parse_script(Path::new(&file))?
            };
            global.finish(&mut client, &value)
        }
        Command::Run {
            file,
            limit,
            arguments,
        } => {
            let mut client = global.open(options)?;
            let mut expressions = Expressions::new(&mut client, bindings);
            let path = (file != "-").then(|| Path::new(&file));
            let value = expressions.run_script(path, limit, &arguments)?;
            if let Some(message) = result_diagnostic(&value) {
                common::progress::println(message);
            } else if let Some(preview) = client.preview(&value) {
                common::progress::println(format!("result {preview}"));
            }
            global.finish(&mut client, &value)
        }
        Command::Export {
            without_backing,
            expression,
            archive,
            expressions: more,
        } => {
            let mut client = global.open(options.clone())?;
            let mut expressions = Expressions::new(&mut client, bindings);
            let excluded = handles(&mut expressions, &without_backing)?;
            let roots = handles(&mut expressions, &[[expression].as_slice(), &more].concat())?;
            commands::visit(&roots, &excluded, true, Some(&archive), &options)?;
            Ok(0)
        }
        Command::FlatwareExport { expression, path } => {
            let mut client = global.open(options)?;
            let mut expressions = Expressions::new(&mut client, bindings);
            let tree = handles(&mut expressions, &[expression])?.remove(0);
            commands::flatware_export(&tree, &path)?;
            Ok(0)
        }
        Command::Remote(command) => {
            configuration::remote(command)?;
            Ok(0)
        }
        Command::Config {
            global,
            local,
            get,
            unset,
            list,
            arguments,
        } => {
            let command = match (get, unset, list, arguments.as_slice()) {
                (Some(key), None, false, []) => ConfigCommand::Get { key },
                (None, Some(key), false, []) => ConfigCommand::Unset { key },
                (None, None, true, []) | (None, None, false, []) => ConfigCommand::List,
                (None, None, false, [key]) => ConfigCommand::Get { key: key.clone() },
                (None, None, false, [key, value]) => ConfigCommand::Set {
                    key: key.clone(),
                    value: value.clone(),
                },
                _ => return Err("invalid fix config arguments; see 'fix config --help'".into()),
            };
            configuration::config(global, local, command)?;
            Ok(0)
        }
        Command::Clone { address, directory } => {
            let scheduler = options
                .remote_schedulers
                .iter()
                .find(|(remote, _)| remote == "origin")
                .map(|(_, name)| name.as_str());
            let collector = options
                .remote_collectors
                .iter()
                .find(|(remote, _)| remote == "origin")
                .map(|(_, name)| name.as_str());
            fixcli::remotes::clone(&address, &directory, scheduler, collector)?;
            Ok(0)
        }
        Command::Pull {
            remote,
            source,
            destination,
            mode,
        } => {
            fixcli::remotes::pull(
                &mut global.open(options)?,
                remote.as_deref(),
                source.as_deref(),
                destination.as_deref(),
                mode.resolve("pull")?,
            )?;
            Ok(0)
        }
        Command::Push {
            remote,
            source,
            destination,
            mode,
        } => {
            fixcli::remotes::push(
                &mut global.open(options)?,
                &remote,
                source.as_deref(),
                destination.as_deref(),
                mode.resolve("push")?,
            )?;
            Ok(0)
        }
        Command::Trust(command) => {
            configuration::trust(command)?;
            Ok(0)
        }
        Command::Init => {
            commands::init()?;
            Ok(0)
        }
        Command::Bootstrap { seed } => {
            if !options.unsigned
                && options.identities.is_empty()
                && fixcertificates::linux::load()?.identities.is_empty()
            {
                return Err(
                    "configure an SSH signing identity first with 'fix trust add-agent'".into(),
                );
            }
            let seed = seed.canonicalize()?;
            let config = fixcertificates::linux::directory()?;
            let config = if config.is_absolute() {
                config
            } else {
                env::current_dir()?.join(config)
            };
            let bootstrap_repository = fixcli::bootstrap_store::repository()?;
            let directory = bootstrap_repository
                .parent()
                .ok_or("invalid bootstrap repository")?
                .to_owned();
            unsafe { env::set_var("FIX_CONFIG_DIR", config) };
            std::fs::create_dir_all(&directory)?;
            env::set_current_dir(&directory)?;
            match fixrepository::Repository::open(".fix") {
                Ok(_) => (),
                Err(fixrepository::RepositoryError::NotInitialized) => {
                    fixrepository::Repository::init(".fix")?;
                }
                Err(error) => return Err(error.into()),
            }
            commands::run(
                commands::Plumbing::Import {
                    archive: seed,
                    sign: !options.unsigned,
                },
                None,
                options.clone(),
                global.profile,
            )?;
            let compiler = std::fs::read_to_string(".fix/trust")?;
            fixcertificates::linux::update("compiler", compiler.trim(), true)?;
            configuration::trust(configuration::TrustCommand::AddTagger {
                tagger: "collector".into(),
            })?;
            configuration::autoconfigure(&fixrepository::Repository::open(".fix")?)?;
            Ok(0)
        }
        Command::Plumbing(plumbing) => Ok(commands::run(
            plumbing,
            global.label,
            options,
            global.profile,
        )?),
    }
}

fn main() -> ExitCode {
    common::progress::enable();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Pipe(Box::new(
            common::progress::Stderr::default(),
        )))
        .init();
    match run() {
        Ok(status) => ExitCode::from(status),
        Err(error) => {
            common::progress::println(error.to_string());
            ExitCode::from(2)
        }
    }
}
