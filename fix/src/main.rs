#![no_main]
#![no_std]

mod parallel_evaluator;
mod scheduler;

use kernel::host::fs::{self, File, Whence};
use kernel::host::os;
use kernel::prelude::*;

use fix::arca::FixOnArca;
use fix::parser::*;
use fix::*;

extern crate alloc;
use alloc::collections::BTreeMap;

#[kmain]
fn main() {
    let argv = os::argv();

    // Subcommand dispatch: `fix init` | `fix eval <file>` | `fix interpret <file>`.
    match argv.get(1).map(String::as_str) {
        Some("init") => init(),
        Some("eval") => {
            let filename = argv.get(2).expect("fix eval: expected a command file");
            eval_file(filename);
        }
        // test to run the parallel evaluator
        Some("parallel_eval") => {
            let filename = argv.get(2).expect("fix eval: expected a command file");
            eval_file_parallel(filename);
        }
        Some("interpret") => {
            let filename = argv.get(2).expect("fix interpret: expected a program file");
            interpret_file(filename);
        }
        Some(other) => panic!(
            "fix: unknown command '{other}' (expected: init | eval <file> | interpret <file>)"
        ),
        None => panic!("fix: expected a command (init | eval <file> | interpret <file>)"),
    }

    kernel::shutdown();
}

/// `fix init`: create the on-disk `.fix` store with its `objects/` and
/// `labels/` subdirs. `mkdir` maps to host `create_dir_all`, so re-running on an
/// existing store is harmless (matches git's "reinitialized existing repository").
fn init() {
    for dir in [".fix/objects", ".fix/labels"] {
        if let Err(e) = fs::mkdir(dir) {
            println!("fix init: failed to create {dir}: {e:?}");
            kernel::exit(1);
        }
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
                println!("handle:    {x}");
                if let Handle::Object(Object::Blob(blob)) = x {
                    let contents = evaluator.storage().get_blob(blob).unwrap();
                    println!("result is a Blob: {contents:?}");
                    if contents.len() == 8 {
                        let bytes: [u8; 8] = (*contents).try_into().unwrap();
                        let value = u64::from_le_bytes(bytes);
                        println!("\tas a u64: {value}");
                    }
                }
            }
        }
    }
}

fn eval(evaluator: &Evaluator<FixOnArca>, e: &Expr, ctx: &mut BTreeMap<String, Handle>) -> Handle {
    match e {
        Expr::Identifier(x) => *ctx.get(x).expect("undefined identifier"),
        Expr::Number(x) => {
            let bytes = i64::to_le_bytes(*x);
            evaluator.storage().add_blob(&bytes).into()
        }
        Expr::String(x) => {
            let bytes = x.as_bytes();
            evaluator.storage().add_blob(bytes).into()
        }
        Expr::Call { name, args } => {
            let arg_handles: Vec<Handle> = args.iter().map(|x| eval(evaluator, x, ctx)).collect();
            match name.as_str() {
                "create_blob" if let Expr::String(path) = &args.get(0).expect("no path") => {
                    let mut file = File::open(path, true, false, false, false, false).unwrap();
                    let len = file.seek(Whence::End(0));
                    file.seek(Whence::Start(0));
                    let mut buf = vec![0; len as usize];
                    file.read_exact(&mut buf);
                    core::mem::forget(file);
                    evaluator.storage().add_blob(&buf).into()
                }
                "create_tree" => evaluator.storage().add_tree(&arg_handles).into(),
                "create_application_thunk" => {
                    Thunk::Application(arg_handles[0].unwrap_object().unwrap_tree()).into()
                }
                "create_strict_encode" => Encode::Strict(arg_handles[0].unwrap_thunk()).into(),
                "eval" => evaluator.eval(arg_handles[0]),
                name => todo!("call {name} {args:?}"),
            }
        }
        Expr::IdentificationThunk(x) => {
            Thunk::Identification(eval(evaluator, x, ctx).unwrap_ref()).into()
        }
        Expr::Ref(reference) => evaluator.lower(eval(evaluator, reference, ctx)),
        Expr::Group(x) => eval(evaluator, x, ctx),
    }
}

// Jennifer: tons of redundancy but I just didn't want to change original code,
// in case errors showed up
// the main change is just calling the parallel evaluator and how its passed in
fn eval_file_parallel(filename: &str) {
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
    let evaluator = parallel_evaluator::Evaluator::new(runtime);

    let mut context = BTreeMap::new();
    for statement in program {
        match statement {
            Statement::Assign { name, expr } => {
                let result = eval_parallel(evaluator.as_ref(), &expr, &mut context);
                context.insert(name, result);
            }
            Statement::Print(expr) | Statement::Expr(expr) => {
                let x = eval_parallel(evaluator.as_ref(), &expr, &mut context);
                println!("handle:    {x}");
                if let Handle::Object(Object::Blob(blob)) = x {
                    let contents = evaluator.storage().get_blob(blob).unwrap();
                    println!("result is a Blob: {contents:?}");
                    if contents.len() == 8 {
                        let bytes: [u8; 8] = (*contents).try_into().unwrap();
                        let value = u64::from_le_bytes(bytes);
                        println!("\tas a u64: {value}");
                    }
                }
            }
        }
    }
}

fn eval_parallel(
    evaluator: &parallel_evaluator::Evaluator<FixOnArca>,
    e: &Expr,
    ctx: &mut BTreeMap<String, Handle>,
) -> Handle {
    match e {
        Expr::Identifier(x) => *ctx.get(x).expect("undefined identifier"),
        Expr::Number(x) => {
            let bytes = i64::to_le_bytes(*x);
            evaluator.storage().add_blob(&bytes).into()
        }
        Expr::String(x) => {
            let bytes = x.as_bytes();
            evaluator.storage().add_blob(bytes).into()
        }
        Expr::Call { name, args } => {
            let arg_handles: Vec<Handle> = args
                .iter()
                .map(|x| eval_parallel(evaluator, x, ctx))
                .collect();
            match name.as_str() {
                "create_blob" if let Expr::String(path) = &args.get(0).expect("no path") => {
                    let mut file = File::open(path, true, false, false, false, false).unwrap();
                    let len = file.seek(Whence::End(0));
                    file.seek(Whence::Start(0));
                    let mut buf = vec![0; len as usize];
                    file.read_exact(&mut buf);
                    core::mem::forget(file);
                    evaluator.storage().add_blob(&buf).into()
                }
                "create_tree" => evaluator.storage().add_tree(&arg_handles).into(),
                "create_application_thunk" => {
                    Thunk::Application(arg_handles[0].unwrap_object().unwrap_tree()).into()
                }
                "create_strict_encode" => Encode::Strict(arg_handles[0].unwrap_thunk()).into(),
                "eval" => evaluator.eval(arg_handles[0]),
                name => todo!("call {name} {args:?}"),
            }
        }
        Expr::IdentificationThunk(x) => {
            Thunk::Identification(eval_parallel(&evaluator, x, ctx).unwrap_ref()).into()
        }
        Expr::Ref(reference) => evaluator.lower(eval_parallel(&evaluator, reference, ctx)),
        Expr::Group(x) => eval_parallel(&evaluator, x, ctx),
    }
}

fn interpret_file(filename: &str) {
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

    let mut interpreter = Interpreter::new(evaluator.storage());
    interpreter.interpret_program(program);
}
