#![no_main]
#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(crate::testing::test_runner)]
#![reexport_test_harness_main = "test_main"]

mod parallel_evaluator;
mod scheduler;
use kernel::host::fs::{File, Whence};
use kernel::host::os;
use kernel::prelude::*;

use fix::arca::FixOnArca;
use fix::parser::*;
use fix::runtime::Runtime;
use fix::storage::disk::DiskStorage;
use fix::*;

#[cfg(test)]
mod testing;

#[cfg(test)]
#[kmain]
fn tests() {
    test_main();
}

#[cfg_attr(not(test), kmain)]
#[cfg_attr(test, allow(dead_code))]
fn main() {
    let argv = os::argv();

    // Subcommand dispatch: `fix init` | `fix create-blob <file>` | `fix eval <file>`.
    match argv.get(1).map(String::as_str) {
        Some("init") => init(),
        Some("create-blob") => {
            let filename = argv.get(2).expect("fix create-blob: expected a file");
            create_blob(filename);
        }
        Some("eval") => {
            let path = argv.get(2).expect("fix eval: expected a command file");
            eval_file(path)
        }
        // test to run the parallel evaluator
        Some("parallel_eval") => {
            let path = argv.get(2).expect("fix eval: expected a command file");
            eval_file_parallel(path);
        }
        Some(other) => panic!(
            "fix: unknown command '{other}' (expected: init | create-blob <file> | eval <file> | parallel_eval <file>)"
        ),
        None => panic!(
            "fix: expected a command (init | create-blob <file> | eval <file> | parallel_eval <file>)"
        ),
    }

    kernel::shutdown();
}

/// `fix init`: initialize the on-disk `.fix` store.
fn init() {
    if let Err(error) = DiskStorage::try_new() {
        println!("fix init: failed to initialize DiskStorage: {error:?}");
        kernel::exit(1);
    }
    let current_dir = match os::current_dir() {
        Ok(path) => path,
        Err(error) => {
            println!("fix init: cannot resolve the current directory: {error:?}");
            kernel::exit(1);
        }
    };
    if current_dir == "/" {
        println!("initialized empty fix store in /.fix");
    } else {
        println!("initialized empty fix store in {current_dir}/.fix");
    }
}

/// `fix create-blob <file>`: content-address the file's bytes, persist an out-of-line
/// blob under `.fix/objects`, and print its canonical handle. Small blobs are
/// represented directly by an inline literal handle and require no object file.
fn create_blob(filename: &str) {
    let mut file = File::open(filename, true, false, false, false, false)
        .unwrap_or_else(|e| panic!("fix create-blob: cannot open {filename}: {e:?}"));
    let len = file.seek(Whence::End(0)) as usize;
    file.seek(Whence::Start(0));
    let mut buf = vec![0; len];
    file.read_exact(&mut buf);

    let source: FixOnArca = FixOnArca::default();
    let machine = match source.storage().add_blob(&buf) {
        Ok(blob) => Handle::from(blob),
        Err(error) => {
            println!("fix create-blob: cannot create machine blob: {error:?}");
            kernel::exit(1);
        }
    };
    let destination = DiskStorage;
    match destination.import(source.storage(), machine) {
        Ok(canonical) => println!("{canonical}"),
        Err(error) => {
            println!("fix create-blob: export failed: {error:?}");
            kernel::exit(1);
        }
    }
}

// Jennifer: tons of redundancy but I just didn't want to change original code,
// in case errors showed up
// the main change is just calling the parallel evaluator and how its passed in
fn eval_file_parallel(path: &str) {
    let file = preprocessor::read_file(path).unwrap();

    let evaluator = parallel_evaluator::Evaluator::new(FixOnArca::default());
    let result = eval_parallel_program(core::str::from_utf8(&file).unwrap(), &evaluator);

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

fn eval_parallel_program(
    source: &str,
    evaluator: &Arc<parallel_evaluator::Evaluator<FixOnArca>>,
) -> Handle {
    let processed = Preprocessor::new(source).preprocess().unwrap();
    let tokens = Lexer::new(&processed).tokenize().unwrap();
    let program = Parser::new(&tokens).parse_program().unwrap();

    let mut interpreter = Interpreter::new(evaluator.storage());
    evaluator.eval(interpreter.interpret(&program))
}

// `fix eval <file>`: read command file and print result.
fn eval_file(path: &str) {
    let file = preprocessor::read_file(path).unwrap();
    let evaluator = Evaluator::new(FixOnArca::default());
    let result = eval_program(core::str::from_utf8(&file).unwrap(), &evaluator);

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

// parse, interpret, and evaluate source text.
fn eval_program(source: &str, evaluator: &Evaluator<FixOnArca>) -> Handle {
    let processed = Preprocessor::new(source).preprocess().unwrap();
    let tokens = Lexer::new(&processed).tokenize().unwrap();
    let program = Parser::new(&tokens).parse_program().unwrap();

    let mut interpreter = Interpreter::new(evaluator.storage());
    evaluator.eval(interpreter.interpret(&program))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval_value(source: &str, evaluator: &Evaluator<FixOnArca>) -> Vec<u8> {
        match eval_program(source, evaluator) {
            Handle::Object(Object::Blob(blob)) => {
                evaluator.storage().get_blob(blob).unwrap().into()
            }
            Handle::Object(Object::Tree(tree)) => tree.len().to_le_bytes().into(),
            Handle::Ref(Ref::Blob(blob)) => evaluator.storage().get_blob(blob).unwrap().into(),
            Handle::Ref(Ref::Tree(tree)) => tree.len().to_le_bytes().into(),
            _ => panic!(),
        }
    }

    #[test_case]
    fn test_number() {
        let evaluator = Evaluator::new(FixOnArca::default());

        {
            assert_eq!(eval_value("42", &evaluator), 42i64.to_le_bytes());
            assert_eq!(eval_value("-1", &evaluator), (-1i64).to_le_bytes());
            assert_eq!(eval_value("\"hello\"", &evaluator), b"hello");

            assert_eq!(eval_value("(1 2 3)", &evaluator), 3i64.to_le_bytes());
            assert_eq!(eval_value("()", &evaluator), 0i64.to_le_bytes());

            assert_eq!(eval_value("&\"hello\"", &evaluator), b"hello");
            assert_eq!(eval_value("&(1 2 3)", &evaluator), 3i64.to_le_bytes());

            assert_eq!(eval_value("!^&2", &evaluator), 2i64.to_le_bytes());

            assert_eq!(
                eval_value("(let ((x 42)) x)", &evaluator),
                42i64.to_le_bytes()
            );
            assert_eq!(
                eval_value("(let ((x 1) (y 2)) (x y))", &evaluator),
                2i64.to_le_bytes()
            );

            assert_eq!(
                eval_value("(let ((x 1)) (let ((x 2)) x))", &evaluator),
                2i64.to_le_bytes()
            );
            assert_eq!(
                eval_value("(let ((x 1)) (let ((y (let ((x 2)) x))) x))", &evaluator),
                1i64.to_le_bytes()
            );
        }
    }
}
