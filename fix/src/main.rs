#![no_main]
#![no_std]
use kernel::host::fs::{self, File, Whence};
use kernel::host::os;
use kernel::prelude::*;

use fix::arca::FixOnArca;
use fix::parser::*;
use fix::*;

extern crate alloc;
use alloc::collections::BTreeMap;

use derive_more::Unwrap;

#[kmain]
fn main() {
    let argv = os::argv();

    // Subcommand dispatch: `fix init` | `fix eval <file>`.
    match argv.get(1).map(String::as_str) {
        Some("init") => init(),
        Some("eval") => {
            let filename = argv.get(2).expect("fix eval: expected a command file");
            eval_file(filename);
        }
        Some(other) => panic!("fix: unknown command '{other}' (expected: init | eval <file>)"),
        None => panic!("fix: expected a command (init | eval <file>)"),
    }

    kernel::shutdown();
}

/// `fix init`: create the on-disk `.fix` store.
/// `mkdir` maps to host `create_dir_all`, so re-running on an existing store
/// is harmless (matches git's "reinitialized existing repository").
fn init() {
    if !fs::mkdir(".fix") {
        panic!("fix init: failed to create .fix");
    }
    println!("initialized empty fix store in .fix");
}

/// `fix eval <file>`: read, parse, and evaluate a command file.
fn eval_file(filename: &str) {
    let mut file = File::open(filename, true, false, false, false, false).unwrap();
    let len = file.seek(Whence::End(0)) as usize;
    file.seek(Whence::Start(0));
    let mut buf = vec![0; len];
    file.read_exact(&mut buf);

    let file = core::str::from_utf8(&buf).unwrap();

    let lexer = Lexer::new(&file);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(&tokens);
    let program = parser.parse_program().unwrap();

    let runtime = FixOnArca::default();
    let evaluator = Evaluator::new(runtime);

    let mut context = BTreeMap::new();
    for statement in program {
        match statement {
            Statement::Assign { name, expr } => {
                let result = eval(&evaluator, &expr, &mut context);
                context.insert(name, result);
            }
            Statement::Print(expr) | Statement::Expr(expr) => {
                let x = eval(&evaluator, &expr, &mut context);
                match x {
                    Value::Handle(x) => {
                        println!("handle:    {x}");
                        if let Some(blob) = x
                            .try_unwrap_object()
                            .ok()
                            .and_then(|x| x.try_unwrap_blob().ok())
                        {
                            let contents = evaluator.storage().get_blob(blob).unwrap();
                            println!("result is a Blob: {contents:?}");
                            if contents.len() == 8 {
                                let bytes: [u8; 8] = (*contents).try_into().unwrap();
                                let value = u64::from_le_bytes(bytes);
                                println!("\tas a u64: {value}");
                            }
                        }
                    }
                    Value::Int(x) => {
                        println!("int: {x}");
                    }
                    Value::String(x) => {
                        println!("string: {x}");
                    }
                    Value::Path(x) => {
                        println!("path: {x}");
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, Unwrap)]
#[unwrap(ref)]
enum Value {
    Handle(Handle),
    Int(i64),
    String(String),
    Path(String),
}

fn eval(evaluator: &Evaluator<FixOnArca>, e: &Expr, ctx: &mut BTreeMap<String, Value>) -> Value {
    match e {
        Expr::Number(x) => Value::Int(*x),
        Expr::Identifier(x) => ctx.get(x).expect("undefined identifier").clone(),
        Expr::String(x) => Value::String(x.clone()),
        Expr::Call { name, args } => {
            let args: Vec<Value> = args.into_iter().map(|x| eval(evaluator, x, ctx)).collect();
            match name.as_str() {
                "Int" => args[0].clone(),
                "create_blob" => match args[0] {
                    Value::Handle(_) => panic!("create blob with handle?"),
                    Value::Int(x) => {
                        let bytes = i64::to_le_bytes(x);
                        Value::Handle(evaluator.storage().add_blob(&bytes).into())
                    }
                    Value::String(ref x) => {
                        Value::Handle(evaluator.storage().add_blob(x.as_bytes()).into())
                    }
                    Value::Path(ref x) => {
                        let mut file = File::open(x, true, false, false, false, false).unwrap();
                        let len = file.seek(Whence::End(0));
                        file.seek(Whence::Start(0));
                        let mut buf = vec![0; len as usize];
                        file.read_exact(&mut buf);
                        core::mem::forget(file);
                        Value::Handle(evaluator.storage().add_blob(&buf).into())
                    }
                },
                "create_tree" => {
                    let handles: Vec<Handle> = args.into_iter().map(Value::unwrap_handle).collect();
                    Value::Handle(evaluator.storage().add_tree(&handles).into())
                }
                "create_application_thunk" => Value::Handle(
                    Thunk::Application(
                        args[0]
                            .clone()
                            .unwrap_handle()
                            .unwrap_object()
                            .unwrap_tree(),
                    )
                    .into(),
                ),
                "create_strict_encode" => Value::Handle(
                    Encode::Strict(args[0].clone().unwrap_handle().unwrap_thunk()).into(),
                ),
                "eval" => Value::Handle(evaluator.eval(args[0].clone().unwrap_handle())),
                "Path" => match args[0] {
                    Value::String(ref x) => Value::Path(x.clone()),
                    _ => panic!("bad path"),
                },
                name => todo!("call {name} {args:?}"),
            }
        }
        Expr::Group(x) => eval(evaluator, x, ctx),
    }
}
