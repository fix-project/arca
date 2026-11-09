#![no_std]
#![no_main]

use benchmark_cases::Operation;
use benchmark_guest::{batches, fixture, timed};
use core::time::Duration;
use fix::Thunk;
use fix::storage::Storage;
use fix_benchmark::{FixState, fix_program, number};
use kernel::prelude::*;

struct Benchmark(Operation);

impl benchmark_guest::Benchmark for Benchmark {
    fn run(&mut self, iterations: u64) -> Duration {
        let mode = match self.0 {
            Operation::Metadata => 6,
            Operation::Read => 0,
            Operation::AttachedRead => 7,
            Operation::Tree => 1,
            Operation::Invalidate => 2,
            Operation::EvalValue => 3,
            Operation::EvalIdentification => 4,
            Operation::EvalApplication => 5,
        };
        let evaluates = (3..=5).contains(&mode);
        batches(iterations, if evaluates { 32 } else { 512 }, |count| {
            let limit = count as u64 + u64::from(evaluates);
            let state = FixState::new(fixture("SDK"), limit, mode);
            let (elapsed, result) = if mode == 6 {
                let pending = fix_reference::think(
                    &state.runtime,
                    state.combination.into_ref().application(),
                );
                timed(|| fix_reference::think(&state.runtime, Thunk::try_from(pending).unwrap()))
            } else if evaluates {
                let certificate = state.memory.get_tree(state.program).unwrap();
                let identity =
                    fix_program(&state.memory, certificate[0], fix::stdlib::PRIMITIVES[0].1);
                let combination = state.memory.add_tree(&[
                    number(0),
                    state.program.into(),
                    number(limit),
                    number(mode),
                    identity.into(),
                ]);
                let pending =
                    fix_reference::think(&state.runtime, combination.into_ref().application());
                let (elapsed, pending) = timed(|| {
                    let mut pending = pending;
                    for _ in 0..count {
                        pending =
                            fix_reference::think(&state.runtime, Thunk::try_from(pending).unwrap());
                    }
                    pending
                });
                let result =
                    fix_reference::think(&state.runtime, Thunk::try_from(pending).unwrap());
                (elapsed, result)
            } else {
                timed(|| state.run(state.combination))
            };
            let result = state
                .memory
                .get_blob(fix::Blob::try_from(result).unwrap())
                .unwrap();
            let expected = if mode == 6 {
                5
            } else if mode == 0 || mode == 7 || evaluates {
                limit
            } else {
                1
            };
            assert_eq!(result.as_ref(), &expected.to_le_bytes());
            elapsed
        })
    }
}

#[kmain]
fn main() {
    benchmark_guest::run(|index, _| Benchmark(Operation::ALL[index]));
}
