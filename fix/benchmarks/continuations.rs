#![no_std]
#![no_main]

use benchmark_cases::Operation;
use benchmark_guest::{batches, fixture, state_batch, timed};
use core::hint::black_box;
use core::sync::atomic::{Ordering, compiler_fence};
use core::time::Duration;
use fix_benchmark::{FixState, fix_program, number};
use fixruntime::Handle;
use fixruntime::arca::EffectAction;
use fixruntime::storage::Storage;
use kernel::prelude::*;

struct Benchmark {
    operation: Operation,
    size: usize,
}
impl Benchmark {
    fn new(operation: Operation, size: usize) -> Self {
        assert!(size <= 1 << 29);
        Self { operation, size }
    }
}
impl benchmark_guest::Benchmark for Benchmark {
    fn run(&mut self, iterations: u64) -> Duration {
        let operation = self.operation;
        let size = self.size;
        assert!(iterations > 0);
        match operation {
            Operation::FixKernelCaptureResume | Operation::FixKernelDescriptorCaptureResume => {
                batches(iterations, 4096, |count| {
                    let program =
                        if matches!(operation, Operation::FixKernelDescriptorCaptureResume) {
                            fixture("DESCRIPTOR_CONTINUATION")
                        } else {
                            fixture("CONTINUATION")
                        };
                    let state = FixState::new(program, size as u64, 0);
                    let mut start = 0;
                    let mut elapsed = Duration::ZERO;
                    let mut calls = 0;
                    let result = state
                        .runtime
                        .execute_with_effects(state.combination, false, &mut |effect| {
                            assert!(effect.same_encoding(&state.request));
                            if calls == 0 {
                                start = kernel::tsc::read_cycles();
                            }
                            if calls == count {
                                compiler_fence(Ordering::SeqCst);
                                unsafe { core::arch::x86_64::_mm_mfence() };
                                elapsed = kernel::tsc::cycles_to_duration(
                                    kernel::tsc::read_cycles() - start,
                                );
                                return EffectAction::Stop(number(42));
                            }
                            calls += 1;
                            EffectAction::Resume(number(0))
                        })
                        .unwrap();
                    state.check_sum(result, 42);
                    elapsed
                })
            }
            Operation::FixCaptureResume => batches(iterations, state_batch(size), |count| {
                let state = FixState::new(fixture("CONTINUATION"), size as u64, 0);
                let mut continuation = state.capture();
                let elapsed = timed(|| {
                    for _ in 0..count {
                        continuation = black_box(state.resume(continuation));
                    }
                })
                .0;
                state.resume(continuation);
                elapsed
            }),
            Operation::FixFunctionEffectRoundTrip => {
                batches(iterations, state_batch(size), |count| {
                    let limit = count as u64 + 1;
                    let state = FixState::new(fixture("FUNCTION_EFFECT"), size as u64, limit);
                    let program = state.memory.get_tree(state.program).unwrap();
                    let program = fix_program(&state.memory, program[0], fixture("EFFECT_HANDLER"));
                    let combination = state.memory.add_tree(&[number(0), program.into()]);
                    let (ready, handler) = state.pair(state.run(combination));
                    state.check_sum(ready, 0);
                    let pending = state.run(state.combination);
                    state.continuation(pending);
                    let (elapsed, (pending, handler)) = timed(|| {
                        let mut pending = pending;
                        let mut handler = handler;
                        for _ in 0..count {
                            (pending, handler) = black_box(state.function_effect(handler, pending));
                        }
                        (pending, handler)
                    });
                    state.check_sum(state.function_effect(handler, pending).0, limit);
                    elapsed
                })
            }
            Operation::FixCanonicalize => batches(iterations, 1, |_| {
                let state = FixState::new(fixture("CONTINUATION"), size as u64, 0);
                let continuation = state.capture();
                let (elapsed, canonical) =
                    timed(|| state.memory.canonicalize(continuation).unwrap());
                let canonical: Handle = canonical.into();
                assert!(canonical.is_canonical());
                state.resume(canonical);
                elapsed
            }),
            Operation::FixReplay => batches(iterations, state_batch(size), |count| {
                let state = FixState::new(fixture("CONTINUATION"), size as u64, 0);
                let continuation: Handle =
                    state.memory.canonicalize(state.capture()).unwrap().into();
                let (elapsed, result) = timed(|| {
                    let mut result = continuation;
                    for _ in 0..count {
                        result = black_box(state.resume(continuation));
                    }
                    result
                });
                state.resume(result);
                elapsed
            }),
        }
    }
}

#[kmain]
fn main() {
    benchmark_guest::run(|index, size| Benchmark::new(Operation::ALL[index], size));
}
