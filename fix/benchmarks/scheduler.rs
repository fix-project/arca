#![no_std]
#![no_main]

use benchmark_cases::Operation;
use benchmark_guest::{batches, fixture, timed};
use core::time::Duration;
use fix::arca::{FixOnArca, trust::Trust};
use fix::storage::{Storage, memory::MemoryStorage};
use fix::user_scheduler_driver::{self, PersistenceOnly};
use fix::{Blob, Handle, Tree};
use fix_benchmark::{fix_program, number};
use kernel::prelude::*;

struct Benchmark;

impl benchmark_guest::Benchmark for Benchmark {
    fn run(&mut self, iterations: u64) -> Duration {
        batches(iterations, 1, |_| {
            let memory = Arc::new(MemoryStorage::default());
            let compiler = memory
                .add_blob(fix::stdlib::PRIMITIVES[0].1)
                .object()
                .into_ref();
            let authority: Handle = memory.canonicalize(compiler.into()).unwrap().into();
            let mut trust = Trust::parse(&format!("{authority}\n")).unwrap();
            let scheduler = fix_program(&memory, compiler.into(), fixture("SCHEDULER"));
            let collector = fix_program(&memory, compiler.into(), fixture("COLLECTOR"));
            trust.add_tagger(&*memory, collector).unwrap();
            let identity = fix_program(&memory, compiler.into(), fix::stdlib::PRIMITIVES[0].1);
            let mut values = Vec::new();
            let mut expected = Vec::new();
            for value in 0..8 {
                let output = number(value);
                let combination = memory.add_tree(&[number(0), identity.into(), output]);
                values.push(combination.into_ref().application().strict().into());
                expected.push(combination.into());
            }
            let input = memory.add_tree(&values);
            let expected = memory.add_tree(&expected);
            let combination = memory.add_tree(&[
                Blob::literal(b"eval").unwrap().into(),
                scheduler.into(),
                collector.into(),
                input.into(),
            ]);
            let runtime = Arc::new(FixOnArca::new(memory.clone()).with_trust(trust));
            let (elapsed, report) = timed(|| {
                user_scheduler_driver::start(
                    runtime,
                    combination,
                    1,
                    PersistenceOnly(|value| Ok(value)),
                )
                .unwrap()
            });
            let coupon = Tree::try_from(report.result.unwrap()).unwrap();
            let claim = memory.certificate_claim(coupon).unwrap();
            let actual = memory.get_tree(claim).unwrap()[2];
            assert!(memory.equals(actual, expected.into()).unwrap());
            elapsed
        })
    }
}

#[kmain]
fn main() {
    benchmark_guest::run(|index, _| match Operation::ALL[index] {
        Operation::EvalEight => Benchmark,
    });
}
