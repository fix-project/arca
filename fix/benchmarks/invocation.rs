#![no_std]
#![no_main]

use benchmark_cases::Operation;
use benchmark_guest::{batches, fixture, timed};
use core::hint::black_box;
use core::time::Duration;
use fix::Tree as FixTree;
use fix::runtime::Runtime as _;
use fix::storage::Storage;
use fix_benchmark::{FixState, number};
use kernel::prelude::*;

struct Benchmark {
    operation: Operation,
}
impl Benchmark {
    fn new(operation: Operation, _size: usize) -> Self {
        Self { operation }
    }
}
impl benchmark_guest::Benchmark for Benchmark {
    fn run(&mut self, iterations: u64) -> Duration {
        let operation = self.operation;
        assert!(iterations > 0);
        match operation {
            Operation::FixInvoke | Operation::FixInvokeCertified => {
                let certified = matches!(operation, Operation::FixInvokeCertified);
                batches(iterations, 4096, |count| {
                    let state = FixState::new(fixture("INVOKE"), 5, 7);
                    state.check_sum(state.run(state.combination), 12);
                    let (elapsed, result) = timed(|| {
                        let mut result = number(0);
                        for _ in 0..count {
                            result = black_box(if certified {
                                state.runtime.execute(state.combination).unwrap().into()
                            } else {
                                state.run(state.combination)
                            });
                        }
                        result
                    });
                    let result = if certified {
                        let claim = state
                            .memory
                            .certificate_claim(FixTree::try_from(result).unwrap())
                            .unwrap();
                        state.memory.get_tree(claim).unwrap()[2]
                    } else {
                        result
                    };
                    state.check_sum(result, 12);
                    elapsed
                })
            }
            Operation::FixChain => batches(iterations, 1, |_| {
                let state = FixState::new(fixture("INVOKE"), 5, 7);
                let (elapsed, result) = timed(|| {
                    let mut value = number(0);
                    for _ in 0..16 {
                        let combination = state.memory.add_tree(&[
                            number(0),
                            state.program.into(),
                            value,
                            number(1),
                        ]);
                        value = black_box(state.run(combination));
                    }
                    value
                });
                state.check_sum(result, 16);
                elapsed
            }),
        }
    }
}

#[kmain]
fn main() {
    benchmark_guest::run(|index, size| Benchmark::new(Operation::ALL[index], size));
}
