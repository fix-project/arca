mod repl;

use clap::{Args, CommandFactory, Parser, Subcommand};
use fixapi::Fix;
use fixcli::{Client, Memoization, Options, Value, commands, expressions::Expressions};
use std::{
    env,
    io::Write,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    process::ExitCode,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Evaluates Fix expressions and manages Fix repositories.
///
/// With no command, parses EXPRESSION, evaluates its imports, and prints the
/// resulting handle. Expression imports are @handle, @label, @file, @script,
/// and @todo.
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
    /// Evaluation parallelism [default: all CPUs]
    #[arg(short, long, value_name = "N", env = "FIX_JOBS", global = true)]
    jobs: Option<NonZeroUsize>,
    /// VM memory budget in MiB, a power of two.
    #[arg(long, value_name = "MIB", default_value_t = 16384, global = true)]
    memory: usize,
    /// Persist coupons: auto, always, or never.
    #[arg(long, value_name = "MODE", default_value_t = Memoization::Auto, global = true)]
    memoize: Memoization,
    /// Scheduler Runnable.
    #[arg(
        long,
        value_name = "NAME",
        env = "FIX_SCHEDULER",
        default_value = "scheduler",
        global = true
    )]
    scheduler: String,
    /// Coupon collector.
    #[arg(
        long,
        value_name = "NAME",
        env = "FIX_COLLECTOR",
        default_value = "collector",
        global = true
    )]
    collector: String,
    /// Report evaluation timings on stderr.
    #[arg(long, global = true)]
    profile: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Create a repository. With SEED, import it and add the expression parser
    /// (label `fix-parser`) into the repository.
    Init {
        /// Bootstrap seed archive to import.
        seed: Option<PathBuf>,
        /// Also compile, with SEED's compiler, the modules `init` adds: the expression
        /// parser. Later commands
        /// reuse these compilations; without this flag, each compiles on first use.
        #[arg(long, requires = "seed")]
        precompile: bool,
    },
    /// Read and evaluate low-level Fix expressions interactively.
    Repl,
    /// Evaluate an expression.
    Eval { expression: String },
    /// Take one Think step.
    Think { expression: String },
    /// Compile a Wasm module with `compile`.
    Compile {
        expression: String,
        /// Number of C translation units [default: one per 128 KiB of Wasm]
        units: Option<u64>,
    },
    /// Record or list where Todo values can be fetched.
    #[command(subcommand)]
    Todo(Todo),
    /// Parse a script (`-` for stdin) without evaluating it.
    Parse { file: String },
    /// Parse and evaluate a script (`-` for stdin).
    Run { file: String },
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

impl Global {
    fn options(&self) -> Result<Options> {
        Ok(Options {
            scheduler: self.scheduler.clone(),
            collector: self.collector.clone(),
            jobs: match self.jobs {
                Some(jobs) => jobs,
                None => std::thread::available_parallelism()?,
            }
            .get(),
            memory_mib: self.memory,
            memoize: self.memoize,
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
        println!("{}", client.save(value)?);
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

fn run() -> Result<u8> {
    let Cli {
        global,
        command,
        expression,
    } = Cli::parse();
    for directory in &global.directories {
        env::set_current_dir(directory)?;
    }
    let options = global.options()?;
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
    if !bindings.is_empty() && matches!(command, Command::Init { .. } | Command::Plumbing(_)) {
        return Err("this command takes no -b bindings".into());
    }
    if global.label.is_some()
        && matches!(
            command,
            Command::Init { .. }
                | Command::Todo(_)
                | Command::Export { .. }
                | Command::FlatwareExport { .. }
                | Command::Repl
        )
    {
        return Err("-l labels an expression result".into());
    }
    match command {
        Command::Repl => repl::run(global.open(options)?, bindings),
        Command::Eval { ref expression }
        | Command::Think { ref expression }
        | Command::Compile { ref expression, .. } => {
            let mut client = global.open(options)?;
            let mut expressions = Expressions::new(&mut client, bindings);
            let input = one(&mut expressions, expression)?;
            let value = match command {
                Command::Eval { .. } => client.eval(&input)?,
                Command::Think { .. } => client.think(&input)?,
                Command::Compile { units, .. } => {
                    let input = if client.value_type(&input)? == fixapi::ValueType::Thunk {
                        client.strict(&input)?
                    } else {
                        input
                    };
                    client.compilation(&input, units)?
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
        Command::Parse { ref file } | Command::Run { ref file } => {
            let mut client = global.open(options)?;
            let mut expressions = Expressions::new(&mut client, bindings);
            let parsed = if file == "-" {
                let source = std::io::read_to_string(std::io::stdin())?;
                expressions.parse(&source, &env::current_dir()?)?
            } else {
                expressions.parse_script(Path::new(file))?
            };
            let value = match command {
                Command::Run { .. } => client.eval(&parsed)?,
                _ => parsed,
            };
            global.finish(&mut client, &value)
        }
        Command::Export {
            without_backing,
            expression,
            archive,
            expressions: more,
        } => {
            let mut client = global.open(options)?;
            let mut expressions = Expressions::new(&mut client, bindings);
            let excluded = handles(&mut expressions, &without_backing)?;
            let roots = handles(&mut expressions, &[[expression].as_slice(), &more].concat())?;
            commands::visit(&roots, &excluded, true, Some(&archive))?;
            Ok(0)
        }
        Command::FlatwareExport { expression, path } => {
            let mut client = global.open(options)?;
            let mut expressions = Expressions::new(&mut client, bindings);
            let tree = handles(&mut expressions, &[expression])?.remove(0);
            commands::flatware_export(&tree, &path)?;
            Ok(0)
        }
        Command::Init { seed, precompile } => {
            commands::init()?;
            let Some(seed) = seed else {
                return Ok(0);
            };
            commands::run(
                commands::Plumbing::Import { archive: seed },
                None,
                options.clone(),
                global.profile,
            )?;
            let options = if precompile {
                Options {
                    trust: Some(env::current_dir()?.join(".fix/trust")),
                    ..options
                }
            } else {
                options
            };
            let mut client = global.open(options)?;
            let parser = fixcli::expressions::install(&mut client)?;
            let compilations = vec![("the expression parser".to_owned(), parser)];
            if precompile {
                eprintln!("fix: precompiling {} modules", compilations.len());
                let requests: Vec<_> = compilations
                    .into_iter()
                    .map(|(_, request)| request)
                    .collect();
                client.precompile(&requests)?;
            }
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
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    match run() {
        Ok(status) => ExitCode::from(status),
        Err(error) => {
            eprintln!("fix: {error}");
            ExitCode::from(2)
        }
    }
}
