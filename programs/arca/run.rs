#![no_std]
#![no_main]

use common::elfloader;
use kernel::host::{fs, os};
use kernel::prelude::*;

const PROGRAM: &[u8] = include_bytes!(concat!(env!("ARCA_PROGRAM_NAME"), "_elf"));

fn read_file(path: &str) -> Vec<u8> {
    let mut file =
        fs::File::open(path, true, false, false, false, false).expect("could not open input file");
    let len = file.seek(fs::Whence::End(0)) as usize;
    file.seek(fs::Whence::Start(0));
    let mut bytes = vec![0; len];
    assert_eq!(file.read_exact(&mut bytes), len);
    bytes
}

fn parse_value(spec: &str) -> Value {
    if spec == "null" {
        return Null::new().into();
    }
    let (kind, value) = spec
        .split_once(':')
        .expect("expected word:, blob:, tuple:, or elf:");
    match kind {
        "word" => Word::new(value.parse().expect("invalid word value")).into(),
        "blob" => Blob::new(value).into(),
        "tuple" => {
            let parts: Vec<&str> = if value.is_empty() {
                Vec::new()
            } else {
                value.split(',').collect()
            };
            let mut items = Tuple::new(parts.len());
            for (index, part) in parts.iter().enumerate() {
                items.set(index, parse_value(part));
            }
            items.into()
        }
        "elf" => elfloader::load_elf(&read_file(value)).unwrap().into(),
        _ => panic!("unknown Arca argument: {spec}"),
    }
}

fn path(blob: &Blob) -> &str {
    core::str::from_utf8(blob).expect("path is not UTF-8")
}

fn print_value(value: Value) {
    match value {
        Value::Null(_) => print!("null"),
        Value::Word(word) => print!("{}", word.read()),
        Value::Blob(blob) => match core::str::from_utf8(&blob) {
            Ok(text) => print!("{text:?}"),
            Err(_) => print!("{blob:?}"),
        },
        Value::Tuple(tuple) => {
            print!("(");
            for (index, item) in tuple.into_iter().enumerate() {
                if index != 0 {
                    print!(", ");
                }
                print_value(item);
            }
            print!(")");
        }
        Value::Function(_) => print!("<function>"),
        Value::Page(_) => print!("<page>"),
        Value::Table(_) => print!("<table>"),
    }
}

fn effect(function: Function) -> Option<Function> {
    let mut data: Tuple = function.read().try_into().expect("invalid effect");
    let tag: Blob = data.take(0).try_into().unwrap();
    assert_eq!(&*tag, b"Symbolic");
    let name: Blob = data.take(1).try_into().unwrap();
    if &*name != b"effect" {
        println!("result: symbolic {name:?}");
        return None;
    }
    let args: Tuple = data.take(2).try_into().unwrap();
    let mut args: Vec<Value> = args.into_iter().collect();
    let continuation: Function = args.pop().unwrap().try_into().unwrap();
    match args.as_slice() {
        [Value::Blob(operation), Value::Blob(input)] if &**operation == b"read" => {
            Some(continuation.apply(Blob::new(read_file(path(input)))))
        }
        [
            Value::Blob(operation),
            Value::Blob(output),
            Value::Blob(bytes),
        ] if &**operation == b"write" => {
            let mut file = fs::File::open(path(output), false, true, true, false, true)
                .expect("could not open output file");
            assert_eq!(file.write_exact(bytes), bytes.len());
            Some(continuation.apply(None))
        }
        [Value::Blob(operation)] if &**operation == b"exit" => None,
        _ => panic!("unhandled Arca effect: {args:?}"),
    }
}

#[kmain]
fn main() {
    let argv = os::argv();
    let mut function: Function = elfloader::load_elf(PROGRAM).unwrap();
    for spec in &argv[1..] {
        function = function.apply(parse_value(spec));
    }
    loop {
        match function.force() {
            Value::Function(next) if next.is_symbolic() => {
                let Some(continuation) = effect(next) else {
                    return;
                };
                function = continuation;
            }
            result => {
                print!("result: ");
                print_value(result);
                println!();
                return;
            }
        }
    }
}
