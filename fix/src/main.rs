#![no_main]
#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(crate::testing::test_runner)]
#![reexport_test_harness_main = "test_main"]

mod parallel_evaluator;
mod scheduler;
use kernel::host::fs;
use kernel::host::os;
use kernel::prelude::*;

use fix::arca::FixOnArca;
use fix::*;

pub const PARSER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fixparser"));

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

    // Subcommand dispatch: `fix init` | `fix eval <file>`.
    match argv.get(1).map(String::as_str) {
        Some("init") => init(),
        Some("eval") => {
            let path = argv.get(2).expect("fix eval: expected a command file");
            eval_file(path)
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
    let parser: Handle = evaluator.storage().add_blob(PARSER).into();
    let source = evaluator.storage().add_blob(processed.as_bytes());
    let environment = stdlib::build_environment(evaluator.storage());
    let combination = evaluator
        .storage()
        .add_tree(&[parser, source.into(), environment]);

    evaluator.eval(Encode::Strict(Thunk::Application(combination)).into())
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
    let parser: Handle = evaluator.storage().add_blob(PARSER).into();
    let source = evaluator.storage().add_blob(processed.as_bytes());
    let environment = stdlib::build_environment(evaluator.storage());
    let combination = evaluator
        .storage()
        .add_tree(&[parser, source.into(), environment]);

    evaluator.eval(Encode::Strict(Thunk::Application(combination)).into())
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

        // primitive test
        {
            assert_eq!(
                eval_value("!*($identity 2)", &evaluator),
                2i64.to_le_bytes()
            );
        }
    }
}
