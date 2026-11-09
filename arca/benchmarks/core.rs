#![no_std]
#![no_main]

use benchmark_cases::Operation;
use benchmark_guest::{fixture, timed};
use core::hint::black_box;
use core::time::Duration;
use kernel::prelude::*;

#[inline(never)]
fn add(a: u64, b: u64) -> u64 {
    a + b
}

fn arca_arguments(mode: u64, first: u64, second: u64) -> Tuple {
    Tuple::from((Word::new(mode), Word::new(first), Word::new(second)))
}

struct Benchmark {
    operation: Operation,
    function: Option<Function>,
    elf: &'static [u8],
}

impl Benchmark {
    fn new(operation: Operation, _size: usize) -> Self {
        let elf = fixture("ARCA");
        let function = matches!(operation, Operation::ArcaInvoke)
            .then(|| common::elfloader::load_elf(elf).unwrap());
        Self {
            operation,
            function,
            elf,
        }
    }
}

impl benchmark_guest::Benchmark for Benchmark {
    fn run(&mut self, iterations: u64) -> Duration {
        let operation = self.operation;
        assert!(iterations > 0);
        match operation {
            Operation::NativeCall => {
                let (elapsed, result) = timed(|| {
                    let mut result = 0;
                    for _ in 0..iterations {
                        result = black_box(add(black_box(5), black_box(7)));
                    }
                    result
                });
                assert_eq!(result, 12);
                elapsed
            }
            Operation::ArcaCreateDestroy => {
                timed(|| {
                    for _ in 0..iterations {
                        drop(black_box(Arca::new()));
                    }
                })
                .0
            }
            Operation::ArcaElfCreateDestroy => {
                timed(|| {
                    for _ in 0..iterations {
                        let function: Function =
                            common::elfloader::load_elf(black_box(self.elf)).unwrap();
                        drop(black_box(function));
                    }
                })
                .0
            }
            Operation::ArcaInvoke => {
                let function = self.function.as_ref().unwrap();
                let (elapsed, result) = timed(|| {
                    let mut result = Value::default();
                    for _ in 0..iterations {
                        result = black_box(function.clone().apply(arca_arguments(0, 5, 7)).force());
                    }
                    result
                });
                let result: Word = result.try_into().unwrap();
                assert_eq!(result.read(), 12);
                elapsed
            }
        }
    }
}

#[kmain]
fn main() {
    benchmark_guest::run(|index, size| Benchmark::new(Operation::ALL[index], size));
}
