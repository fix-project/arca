use common::bitpack::BitPack;
use evaluator::{
    fixruntime::{DeterministicEquivRuntime, Expr, Statement, Value, CouponHelper, Operator},
    hybridruntime::HybridRuntime,
    lexer::Lexer,
    mockruntime::MockRuntime,
    parser::Parser as ExprParser,
};
use fixhandle::rawhandle::{FixHandle, create_application_thunk, create_strict_encode};
use std::{
    collections::BTreeMap,
    env, fmt, fs,
    io::{self, Read},
    process,
};

use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(
    override_usage = "fix-cli hybrid <KERNEL> [--smp <N>] [--cid <N>] -- [--path <FILE> | <commands...>]"
)]
struct Args {
    kernel: PathBuf,
    #[arg(short, long)]
    smp: Option<usize>,
    #[arg(short, long, default_value = "3")]
    cid: usize,
}

fn main() {
    // Skip executable path
    let mut args = env::args().skip(1);

    let runtime_name = match args.next() {
        Some(val) => val,
        None => {
            eprintln!("Error - usage: fix-cli <mock|hybrid> -- <commands...> | --path <file>");
            process::exit(1);
        }
    };

    let (runtime_args, command_args) = split_args(args.collect());
    let result = match runtime_name.as_str() {
        "mock" => match read_commands(command_args) {
            Ok(commands) => run(MockRuntime::new(), &commands),
            Err(error) => {
                eprintln!("Error - {error}");
                process::exit(1);
            }
        },
        "hybrid" => {
            let runtime_config = Args::parse_from(
                std::iter::once(String::from("fix-cli hybrid")).chain(runtime_args),
            );

            let commands = match read_commands(command_args) {
                Ok(commands) => commands,
                Err(error) => {
                    eprintln!("Error - {error}");
                    process::exit(1);
                }
            };

            let smp = runtime_config.smp.unwrap_or(1);
            let cid = runtime_config.cid;

            let bin: Arc<[u8]> = match std::fs::read(runtime_config.kernel) {
                Ok(bin) => bin.into(),
                Err(error) => {
                    eprintln!("Error - {error}");
                    process::exit(1);
                }
            };
            run(HybridRuntime::new(smp, cid, bin), &commands)
        }
        // "storage" => run(StorageRuntime::new(), &commands),
        other => Err(format!("expected 'mock|hybrid' but got '{other}'")),
    };

    if let Err(error) = result {
        eprintln!("Error - {error}");
        process::exit(1);
    }
}

fn split_args(args: Vec<String>) -> (Vec<String>, Vec<String>) {
    match args.iter().position(|arg| arg == "--") {
        Some(idx) => (args[..idx].to_vec(), args[idx + 1..].to_vec()),
        None => (Vec::new(), args),
    }
}

fn run<R>(mut runtime: R, commands: &str) -> Result<(), String>
where
    R: CouponHelper + Operator,
    <R as DeterministicEquivRuntime>::Error: fmt::Debug,
    for<'a> R::BlobData<'a>: AsRef<[u8]>,
    for<'a> R::TreeData<'a>: AsRef<[u8]>,
{
    let (output, output_handle) = evaluate_commands(&mut runtime, commands)?;
    print!("{output}");
    if let Some(h) = output_handle {
        println!("{h:?}");
    }
    Ok(())
}

fn read_commands(args: Vec<String>) -> Result<String, String> {
    let args: Vec<String> = args.into_iter().filter(|arg| arg != "--").collect();

    match args.as_slice() {
        [flag, path] if flag == "--path" => {
            fs::read_to_string(path).map_err(|error| format!("can't read file '{path}': {error}"))
        }
        [flag, ..] if flag == "--path" => Err(String::from("expected path")),
        [] => {
            let mut commands = String::new();
            io::stdin()
                .read_to_string(&mut commands)
                .map_err(|error| error.to_string())?;
            Ok(commands)
        }
        _ => Ok(args.join(" ")),
    }
}

pub fn evaluate_commands<R>(runtime: &mut R, commands: &str) -> Result<(String, Option<FixHandle>), String>
where
    R: Operator + CouponHelper,
    for<'a> R::BlobData<'a>: AsRef<[u8]>,
    for<'a> R::TreeData<'a>: AsRef<[u8]>,
{
    let tokens = Lexer::new(commands).tokenize()?;
    let program = ExprParser::new(&tokens).parse_program()?;
    let mut evaluator = Evaluator::new(runtime);

    let mut output = String::new();
    let mut output_handle: Option<FixHandle> = None;
    for statement in program {
        if let Some(text) = evaluator.evaluate_statement(statement)? {
            match text {
                Value::String(s) => { output.push_str(&s); output.push('\n') },
                Value::Handle(h) => output_handle = Some(h),
                _ => todo!()
            };
        }
    }
    Ok((output, output_handle))
}

type RuntimeValue = Value<FixHandle, Vec<u8>, Vec<u8>>;

struct Evaluator<'a, R: DeterministicEquivRuntime<Handle = FixHandle>> {
    runtime: &'a mut R,
    variables: BTreeMap<String, RuntimeValue>,
}

impl<'a, R> Evaluator<'a, R>
where
    R: CouponHelper + Operator,
    for<'b> R::BlobData<'b>: AsRef<[u8]>,
    for<'b> R::TreeData<'b>: AsRef<[u8]>,
{
    fn new(runtime: &'a mut R) -> Self {
        Self {
            runtime,
            variables: BTreeMap::new(),
        }
    }

    fn evaluate_statement(&mut self, statement: Statement) -> Result<Option<RuntimeValue>, String> 
    {
        match statement {
            Statement::Assign { name, expr } => {
                let value = self.evaluate_expr(expr)?;
                self.variables.insert(name, value);
                Ok(None)
            }
            Statement::Print(expr) => Ok(Some(Value::String(format!("result: {}\n", self.evaluate_expr(expr)?)))),
            Statement::ShowCoupon(expr) => {
                if let Value::Handle(h) = self.evaluate_expr(expr)? {
                    Ok(Some(Value::String(self.runtime.show_coupon(&h))))

                } else {
                    Err("not a coupon to show".to_string())
                }
            }
            Statement::Expr(expr) => {
                let result = self.evaluate_expr(expr)?;
                Ok(Some(result))
            }
        }
    }

    fn evaluate_expr(&mut self, expr: Expr) -> Result<RuntimeValue, String> {
        match expr {
            Expr::Number(number) => Ok(Value::Int(number)),
            Expr::String(string) => Ok(Value::String(string)),
            Expr::Group(expr) => self.evaluate_expr(*expr),
            Expr::Identifier(name) => {
                if name == "mock" || name == "hybrid" {
                    return Ok(Value::Unit);
                }
                self.variables
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| format!("unknown variable: {name}"))
            }
            Expr::Call { name, args } => self.evaluate_call(&name, args),
        }
    }

    fn evaluate_call(&mut self, name: &str, args: Vec<Expr>) -> Result<RuntimeValue, String> {
        match name {
            "create_blob" => {
                let handle = self.evaluate_primitive(name, args)?;
                Ok(Value::Handle(handle))
            }
            "create_tree" => {
                let mut bytes = Vec::with_capacity(args.len() * 32);
                for expr in args {
                    let value = self.evaluate_expr(expr)?;
                    bytes.extend_from_slice(&self.make_handle(name, value)?.pack());
                }
                Ok(Value::Handle(self.runtime.create_tree(&bytes)))
            }
            "create_application_thunk" => {
                let expr = self.evaluate_expr(self.get_arg(name, &args)?.clone())?;
                let handle = self.make_handle(name, expr)?;
                let handle = create_application_thunk(&handle).map_err(|_| "Failed to create application thunk")?;
                Ok(Value::Handle(handle))
            }
            "create_strict_encode" => {
                let expr = self.evaluate_expr(self.get_arg(name, &args)?.clone())?;
                let handle = self.make_handle(name, expr)?;
                let handle = create_strict_encode(&handle).map_err(|_| "Failed to create strict encode")?;
                Ok(Value::Handle(handle))
            }
            "get_blob" => {
                let expr = self.evaluate_expr(self.get_arg(name, &args)?.clone())?;
                let handle = self.make_handle(name, expr)?;
                let blob = self
                    .runtime
                    .get_blob(&handle)
                    .map_err(|error| format!("{name}: {error:?}"))?;
                Ok(Value::BlobData(blob.as_ref().to_vec()))
            }
            "get_tree" => {
                let expr = self.evaluate_expr(self.get_arg(name, &args)?.clone())?;
                let handle = self.make_handle(name, expr)?;
                let tree = self
                    .runtime
                    .get_tree(&handle)
                    .map_err(|e| format!("{name}: {e:?}"))?;
                Ok(Value::TreeData(tree.as_ref().to_vec()))
            }
            "apply" => {
                let expr = self.evaluate_expr(self.get_arg(name, &args)?.clone())?;
                let handle = self.make_handle(name, expr)?;
                let apply_handle = self
                    .runtime
                    .apply(handle);
                Ok(Value::Handle(apply_handle))
            }
            "eval" => {
                let expr = self.evaluate_expr(self.get_arg(name, &args)?.clone())?;
                let handle = self.make_handle(name, expr)?;
                let eval_handle = self
                    .runtime
                    .eval(handle);
                Ok(Value::Handle(eval_handle))
            }
            "print" => self.evaluate_expr(self.get_arg(name, &args)?.clone()),
            "mock" | "hybrid" => {
                if args.is_empty() {
                    Ok(Value::Unit)
                } else {
                    Err(format!("{name}: unexpected args"))
                }
            }
            _ => Err(format!("unknown function: {name}")),
        }
    }

    fn evaluate_primitive(&mut self, name: &str, args: Vec<Expr>) -> Result<R::Handle, String> {
        let Expr::Call { name, args } = self.get_arg(name, &args)? else {
            return Err(String::from("Expected primitive for create_blob()"));
        };

        let [inner] = args.as_slice() else {
            return Err(format!("{name}: primitive takes 1 argument"));
        };
        match self.evaluate_expr(inner.clone())? {
            Value::Int(number) if name == "Int" => {
                Ok(self.runtime.create_blob_i64(number as u64))
            }
            Value::String(string) if name == "String" => {
                Ok(self.runtime.create_blob(string.as_bytes()))
            }
            Value::String(path) => {
                let blob = fs::read(&path)
                    .map_err(|error| format!("{name}: can't read file {path:?}: {error}"))?;
                Ok(self.runtime.create_blob(&blob))
            }
            other => Err(format!(
                "{name}: expected Int(<num>), String(\"text\"), or Path(\"...\"), got {other:?}"
            )),
        }
    }

    fn make_handle(&self, name: &str, value: RuntimeValue) -> Result<FixHandle, String> {
        match value {
            Value::Handle(handle) => Ok(handle),
            other => Err(format!("{name}: got {other} for handle")),
        }
    }

    fn get_arg<'b>(&self, name: &str, args: &'b [Expr]) -> Result<&'b Expr, String> {
        let [expr] = args else {
            return Err(self.wrong_arity(name, 1, args.len()));
        };
        Ok(expr)
    }

    fn wrong_arity(&self, name: &str, expected: usize, found: usize) -> String {
        format!("{name}: got {found} arguments but expected {expected}")
    }
}
