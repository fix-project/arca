#![no_std]
#![no_main]

use benchmark_cases::Operation;
use benchmark_guest::{batches, fixture, state_batch, timed};
use core::hint::black_box;
use core::time::Duration;
use kernel::prelude::*;

fn arca_arguments(mode: u64, first: u64, second: u64) -> Tuple {
    Tuple::from((Word::new(mode), Word::new(first), Word::new(second)))
}

fn arca_transfer(value: Value) -> (Value, Function) {
    let function: Function = value.try_into().expect("expected effect");
    let mut definition: Tuple = function.read().try_into().unwrap();
    let mut arguments: Tuple = definition.take(2).try_into().unwrap();
    let request = arguments.take(0);
    let continuation = arguments.take(1).try_into().unwrap();
    (request, continuation)
}

fn arca_yield(value: Value) -> (Function, u64) {
    let (request, continuation) = arca_transfer(value);
    let epoch: Word = request.try_into().unwrap();
    (continuation, epoch.read())
}

fn arca_function_effect(value: Value, handler: Function) -> (Value, Function) {
    let (request, continuation) = arca_transfer(value);
    let (reply, handler) = arca_transfer(handler.apply(request).force());
    (continuation.apply(reply).force(), handler)
}

struct ArcaState {
    function: Option<Function>,
    epoch: u64,
}

impl ArcaState {
    fn new(size: usize) -> Self {
        let function: Function = common::elfloader::load_elf(fixture("ARCA")).unwrap();
        let (function, epoch) =
            arca_yield(function.apply(arca_arguments(1, size as u64, 0)).force());
        assert_eq!(epoch, 1);
        Self {
            function: Some(function),
            epoch,
        }
    }

    fn step(&mut self, copy: bool) {
        let function = self.function.take().unwrap();
        let snapshot = if copy { Some(function.clone()) } else { None };
        let (function, epoch) = arca_yield(function.apply(Null::new()).force());
        self.function = Some(function);
        self.epoch = epoch;
        drop(snapshot);
    }
}

struct Benchmark {
    operation: Operation,
    size: usize,
    state: Option<ArcaState>,
}

impl Benchmark {
    fn new(operation: Operation, size: usize) -> Self {
        assert!(size <= 1 << 29);
        let state = match operation {
            Operation::ArcaFunctionEffectRoundTrip => None,
            _ => Some(ArcaState::new(size)),
        };
        Self {
            operation,
            size,
            state,
        }
    }
}

impl benchmark_guest::Benchmark for Benchmark {
    fn run(&mut self, iterations: u64) -> Duration {
        let operation = self.operation;
        let size = self.size;
        assert!(iterations > 0);
        match operation {
            Operation::ArcaEffectRoundTrip | Operation::ArcaCopyResumeDirty => {
                let Some(state) = &mut self.state else {
                    unreachable!()
                };
                let epoch = state.epoch;
                let copy = matches!(operation, Operation::ArcaCopyResumeDirty);
                let elapsed = timed(|| {
                    for _ in 0..iterations {
                        state.step(copy);
                    }
                })
                .0;
                assert_eq!(state.epoch, epoch + iterations);
                elapsed
            }
            Operation::ArcaFunctionEffectRoundTrip => {
                batches(iterations, state_batch(size), |count| {
                    let handler =
                        common::elfloader::load_elf(fixture("ARCA_EFFECT_HANDLER")).unwrap();
                    let (ready, handler) = arca_transfer(handler.force());
                    let ready: Word = ready.try_into().unwrap();
                    assert_eq!(ready.read(), 0);
                    let producer =
                        common::elfloader::load_elf(fixture("ARCA_FUNCTION_EFFECT")).unwrap();
                    let limit = count as u64 + 1;
                    let pending = producer
                        .apply(Word::new(size as u64))
                        .apply(Word::new(limit))
                        .force();
                    let (elapsed, (pending, handler)) = timed(|| {
                        let mut pending = pending;
                        let mut handler = handler;
                        for _ in 0..count {
                            (pending, handler) = black_box(arca_function_effect(pending, handler));
                        }
                        (pending, handler)
                    });
                    let result: Word = arca_function_effect(pending, handler).0.try_into().unwrap();
                    assert_eq!(result.read(), limit);
                    elapsed
                })
            }
            Operation::ArcaClone => {
                let Some(state) = &self.state else {
                    unreachable!()
                };
                batches(iterations, state_batch(size), |count| {
                    let mut copies = Vec::with_capacity(count);
                    let elapsed = timed(|| {
                        for _ in 0..count {
                            copies.push(black_box(state.function.as_ref().unwrap().clone()));
                        }
                    })
                    .0;
                    let (_, epoch) = arca_yield(copies.pop().unwrap().apply(Null::new()).force());
                    assert_eq!(epoch, state.epoch + 1);
                    elapsed
                })
            }
        }
    }
}

#[kmain]
fn main() {
    benchmark_guest::run(|index, size| Benchmark::new(Operation::ALL[index], size));
}
