#![no_main]
#![no_std]

mod parallel_evaluator;
mod scheduler;

use kernel::host::fs;
use kernel::host::os;
use kernel::prelude::*;

use fix::arca::FixOnArca;
use fix::parser::*;
use fix::*;

#[kmain]
fn main() {
    let argv = os::argv();

    // Subcommand dispatch: `fix init` | `fix eval <file>` `.
    match argv.get(1).map(String::as_str) {
        Some("init") => init(),
        Some("eval") => {
            let path = argv.get(2).expect("fix eval: expected a command file");
            eval_file(path);
        }
        // test to run the parallel evaluator
        Some("parallel_eval") => {
            let path = argv.get(2).expect("fix eval: expected a command file");
            eval_file_parallel(path);
        }
        Some(other) => panic!("fix: unknown command '{other}' (expected: init | eval <file> )"),
        None => panic!("fix: expected a command (init | eval <file> "),
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
fn eval_file(path: &str) {
    let file = Lexer::read_file(path).unwrap();
    let file = Lexer::preprocess(core::str::from_utf8(&file).unwrap()).unwrap();
    let tokens = Lexer::new(&file).tokenize().unwrap();
    let program = Parser::new(&tokens).parse_program().unwrap();

    let evaluator = Evaluator::new(FixOnArca::default());
    let mut interpreter = Interpreter::new(evaluator.storage());
    let result = evaluator.eval(interpreter.interpret(&program));

    println!("handle:    {result}");
    println!("Current handle is: {:?}", result);
    if let Handle::Object(Object::Blob(blob)) = result {
        let contents = evaluator.storage().get_blob(blob).unwrap();
        println!("result is a Blob: {contents:?}");
        if contents.len() == 8 {
            let bytes: [u8; 8] = (*contents).try_into().unwrap();
            println!("\tas a u64: {}", u64::from_le_bytes(bytes));
        }
    }
}

// Jennifer: tons of redundancy but I just didn't want to change original code,
// in case errors showed up
// the main change is just calling the parallel evaluator and how its passed in
fn eval_file_parallel(path: &str) {
    let file = Lexer::read_file(path).unwrap();
    let file = Lexer::preprocess(core::str::from_utf8(&file).unwrap()).unwrap();
    let tokens = Lexer::new(&file).tokenize().unwrap();
    let program = Parser::new(&tokens).parse_program().unwrap();

    let runtime = FixOnArca::default();
    let evaluator = parallel_evaluator::Evaluator::new(runtime);
    let mut interpreter = Interpreter::new(evaluator.storage());
    let result = evaluator.eval(interpreter.interpret(&program));

    println!("handle:    {result}");
    println!("Current handle is: {:?}", result);
    if let Handle::Object(Object::Blob(blob)) = result {
        let contents = evaluator.storage().get_blob(blob).unwrap();
        println!("result is a Blob: {contents:?}");
        if contents.len() == 8 {
            let bytes: [u8; 8] = (*contents).try_into().unwrap();
            println!("\tas a u64: {}", u64::from_le_bytes(bytes));
        }
    }
}