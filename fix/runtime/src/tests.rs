#![no_main]
#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(kernel::testing::harness)]
#![reexport_test_harness_main = "test_main"]

use fix::arca::FixOnArca;
use fix::*;
#[path = "../tests/advice.rs"]
mod advice;
#[path = "../tests/api.rs"]
mod api;
#[path = "../tests/evaluation.rs"]
mod evaluation;
#[path = "../tests/maps.rs"]
mod maps;
#[path = "../tests/scheduler.rs"]
mod scheduler;
use evaluation::Evaluation;
use kernel::prelude::*;

#[kmain]
fn main() {
    test_main();
}

fn test_runtime() -> FixOnArca {
    let author = fixhandle::Blob::canonical(stdlib::PRIMITIVES[0].1)
        .unwrap()
        .object()
        .into_ref();
    FixOnArca::default()
        .with_trust(fix::arca::trust::Trust::parse(&format!("{}\n", author.as_handle())).unwrap())
}

fn test_program(storage: &dyn fix::storage::Storage, bytes: &[u8]) -> fixhandle::Tree {
    let author = storage
        .add_blob(stdlib::PRIMITIVES[0].1)
        .object()
        .into_ref();
    storage
        .add_tree(&[
            author.into(),
            storage.add_blob(bytes).into(),
            fixhandle::Blob::literal(b"Runnable").unwrap().into(),
        ])
        .tag_descriptor()
}

fn eval_program(source: &str, evaluator: &Evaluation) -> Handle {
    let processed = fixpreprocessor::Preprocessor::new(source)
        .preprocess(|path| {
            kernel::host::fs::File::open(path, true, false, false, false, false)
                .and_then(|mut file| file.try_read_to_end())
                .map_err(|error| format!("{error:?}"))
        })
        .unwrap();
    let parser: Handle =
        test_program(evaluator.storage(), kernel::testing::fixture("FIX_PARSER")).into();
    let source = evaluator.storage().add_blob(processed.as_bytes());
    let environment = stdlib::build_environment(evaluator.storage());
    let combination = evaluator.storage().add_tree(&[
        fixhandle::Blob::literal(b"").unwrap().into(),
        parser,
        source.into(),
        environment,
    ]);

    evaluator.eval(combination.into_ref().application().strict().into())
}

mod tests {
    use super::*;
    use fix::runtime::Runtime;
    use fix::storage::CanonicalHandle;
    use fixhandle::{Blob, Tree};

    fn apply(runtime: &impl Runtime, combination: Tree) -> Handle {
        let coupon = runtime.execute(combination).unwrap();
        let claim = runtime.storage().certificate_claim(coupon).unwrap();
        runtime.storage().get_tree(claim).unwrap()[2]
    }

    #[kernel::test]
    fn exception_handlers_preserve_unchanged_and_modified_parameters() {
        let runtime = test_runtime();
        let storage = runtime.storage();
        let program = test_program(
            storage,
            kernel::testing::fixture("FIX_EXCEPTION_PARAMETERS"),
        );
        let combination = storage.add_tree(&[Blob::literal(b"").unwrap().into(), program.into()]);
        let result = apply(&runtime, combination);
        let bytes = storage.get_blob(Blob::try_from(result).unwrap()).unwrap();
        assert_eq!(bytes.as_ref(), &49u64.to_le_bytes());
    }

    #[kernel::test]
    fn executable_instances_preserve_runtime_isolation_and_repeated_results() {
        let runtimes = [test_runtime(), test_runtime()];
        let mut programs = Vec::new();
        for (index, source) in [
            stdlib::PRIMITIVES[0].1,
            kernel::testing::fixture("FIX_EVAL"),
        ]
        .into_iter()
        .enumerate()
        {
            programs.push((index, test_program(runtimes[index].storage(), source)));
        }
        for input in [7u64, 41] {
            for &(index, program) in programs.iter().rev() {
                let runtime = &runtimes[index];
                let storage = runtime.storage();
                let argument = Blob::literal(&input.to_le_bytes()).unwrap();
                let empty = Blob::literal(b"").unwrap();
                let combination = storage.add_tree(&[
                    empty.into(),
                    program.into(),
                    argument.into(),
                    Blob::literal(&0u64.to_le_bytes()).unwrap().into(),
                ]);
                let output = apply(runtime, combination);
                if index == 0 {
                    assert!(storage.equals(output, combination.into()).unwrap());
                } else {
                    let ThunkView::Application(reference) = Thunk::try_from(output).unwrap().view()
                    else {
                        panic!("expected continuation application")
                    };
                    let pending = storage.get_tree(reference.object_descriptor()).unwrap();
                    assert!(storage.equals(pending[2], argument.into()).unwrap());
                    let resumed = storage.add_tree(&[empty.into(), pending[1], argument.into()]);
                    let result = Blob::try_from(apply(runtime, resumed)).unwrap();
                    assert_eq!(
                        storage.get_blob(result).unwrap().as_ref(),
                        &(input + 1).to_le_bytes()
                    );
                }
            }
        }
    }

    #[kernel::test]
    fn canonical_executable_instances_preserve_store_authority_and_arguments() {
        for (trusted, input) in [(true, 7u64), (true, 41), (false, 99)] {
            let runtime: FixOnArca = if trusted {
                test_runtime()
            } else {
                FixOnArca::default()
            };
            let storage = runtime.storage();
            let program = test_program(storage, stdlib::PRIMITIVES[0].1);
            let canonical: Handle = storage.canonicalize(program.into()).unwrap().into();
            let program = Tree::try_from(canonical).unwrap();
            let combination = storage.add_tree(&[
                Blob::literal(b"").unwrap().into(),
                program.into(),
                Blob::literal(&input.to_le_bytes()).unwrap().into(),
            ]);
            if trusted {
                let result = apply(&runtime, combination);
                assert!(storage.equals(result, combination.into()).unwrap());
            } else {
                assert!(runtime.execute(combination).is_err());
            }
        }
    }

    #[kernel::test]
    fn runnable_authorities() {
        use fix::arca::trust::{Trust, TrustError};

        let runtime = test_runtime();
        let storage = runtime.storage();
        let compiler = storage.add_blob(&[1; 64]);
        let operator = storage.add_blob(&[2; 64]);
        let executable = storage.add_blob(b"executable");
        let canonical_compiler: Handle = storage.canonicalize(compiler.into()).unwrap().into();
        let canonical_operator: Handle = storage.canonicalize(operator.into()).unwrap().into();
        let trust = Trust::parse(&format!("{canonical_compiler}\n")).unwrap();
        let certificate = |author| {
            storage.add_tree(&[
                author,
                executable.into(),
                Blob::literal(b"Runnable").unwrap().into(),
            ])
        };
        let kernel = storage.add_tree(&[]);
        let unconditional = certificate(kernel.into()).tag_descriptor();
        assert!(Trust::default().runnable(storage, unconditional).is_ok());
        let compiled = certificate(compiler.into());
        assert!(matches!(
            trust.runnable(storage, compiled),
            Err(TrustError::InvalidRunnable)
        ));
        assert!(matches!(
            Trust::default().runnable(storage, compiled.tag_descriptor()),
            Err(TrustError::UntrustedAuthority)
        ));
        assert!(trust.runnable(storage, compiled.tag_descriptor()).is_ok());
        let reference = compiler.object().into_ref();
        let canonical_reference: Handle = storage.canonicalize(reference.into()).unwrap().into();
        let reference_trust = Trust::parse(&format!("{canonical_reference}\n")).unwrap();
        assert!(
            reference_trust
                .runnable(storage, certificate(reference.into()).tag_descriptor())
                .is_ok()
        );
        assert!(matches!(
            reference_trust.runnable(storage, compiled.tag_descriptor()),
            Err(TrustError::UntrustedAuthority)
        ));
        let operated = certificate(operator.into()).tag_descriptor();
        assert!(matches!(
            trust.runnable(storage, operated),
            Err(TrustError::UntrustedAuthority)
        ));
        let trust = Trust::parse(&format!("{canonical_compiler}\n{canonical_operator}\n")).unwrap();
        assert!(trust.runnable(storage, operated).is_ok());
        assert!(
            Trust::default()
                .runnable(
                    storage,
                    certificate(kernel.tag_descriptor().into()).tag_descriptor()
                )
                .is_err()
        );
        assert!(Trust::parse(&format!("{}\n", compiler.as_handle())).is_err());
    }

    #[kernel::test]
    fn evaluation_certificates() {
        use fix::coupon::Exchange;

        fn number(value: u64) -> Handle {
            Blob::literal(&value.to_le_bytes()).unwrap().into()
        }

        fn witness(storage: &dyn Storage, coupon: Tree) -> Handle {
            let claim = storage.certificate_claim(coupon).unwrap();
            storage.add_tree(&[coupon.into(), claim.into()]).into()
        }

        fn collect(
            runtime: &FixOnArca,
            collector: Tree,
            exchange: Exchange,
            input: Handle,
            result: Handle,
            witnesses: &[Handle],
        ) -> Handle {
            let storage = runtime.storage();
            let request = storage.add_tree(&[
                Blob::literal(b"").unwrap().into(),
                collector.into(),
                number(exchange as u64),
                input,
                result,
                storage.add_tree(witnesses).into(),
            ]);
            let coupon = Tree::try_from(runtime.execute_privileged(request).unwrap()).unwrap();
            assert!(coupon.is_tag());
            let claim = storage.certificate_claim(coupon).unwrap();
            let assertion = storage.get_tree(claim).unwrap();
            assert!(assertion[0].same_encoding(&number(exchange.statement() as u64)));
            assert!(assertion[1].same_encoding(&input));
            assert!(assertion[2].same_encoding(&result));
            let fields = storage.get_tree(coupon).unwrap();
            assert!(fields[0].same_encoding(&collector.into()));
            witness(storage, coupon)
        }

        fn value(runtime: &FixOnArca, collector: Tree, input: Handle) -> Handle {
            if let Ok(tree) = Tree::try_from(input) {
                let witnesses: Vec<_> = runtime
                    .storage()
                    .get_tree(tree)
                    .unwrap()
                    .iter()
                    .map(|child| value(runtime, collector, *child))
                    .collect();
                collect(runtime, collector, Exchange::Eval, input, input, &witnesses)
            } else {
                collect(runtime, collector, Exchange::Eval, input, input, &[])
            }
        }

        let evaluator = Evaluation::new(test_runtime());
        let runtime = evaluator.runtime();
        let storage = runtime.storage();
        let collector = test_program(storage, kernel::testing::fixture("FIX_COLLECTOR"));
        let collect = |rule, input, result, witnesses: &[Handle]| {
            collect(runtime, collector, rule, input, result, witnesses)
        };
        let value = |input| value(runtime, collector, input);
        let metadata: Handle = Blob::literal(b"").unwrap().into();
        let blob = storage.add_blob(&[42; 64]);
        let reference: Handle = blob.into_ref().into();
        let thunk = blob.into_ref().identification();
        let thought = collect(Exchange::Think, thunk.into(), reference, &[]);
        let forced = collect(Exchange::ThinkToForce, thunk.into(), reference, &[thought]);
        let equivalent = collect(
            Exchange::ForceToEncode,
            thunk.strict().into(),
            blob.into(),
            &[forced],
        );
        let reverse = collect(
            Exchange::Symmetry,
            blob.into(),
            thunk.strict().into(),
            &[equivalent],
        );
        let strict = collect(
            Exchange::EvalEq,
            thunk.strict().into(),
            blob.into(),
            &[value(blob.into()), reverse],
        );

        let program = test_program(storage, stdlib::PRIMITIVES[0].1);
        let combination = storage.add_tree(&[metadata, program.into(), thunk.strict().into()]);
        let ready = storage.add_tree(&[metadata, program.into(), blob.into()]);
        let evaluated = collect(
            Exchange::Eval,
            combination.into(),
            ready.into(),
            &[value(metadata), value(program.into()), strict],
        );
        let applied = witness(storage, runtime.execute(ready).unwrap());
        let application = combination.into_ref().application();
        let thought = collect(
            Exchange::Think,
            application.into(),
            ready.into(),
            &[evaluated, applied],
        );
        let forced = collect(
            Exchange::ThinkToForce,
            application.into(),
            ready.into(),
            &[thought],
        );
        let equivalent = collect(
            Exchange::ForceToEncode,
            application.shallow().into(),
            ready.into_ref().into(),
            &[forced],
        );
        collect(Exchange::Reflexivity, reference, reference, &[]);
        collect(
            Exchange::Transitivity,
            application.shallow().into(),
            ready.into_ref().into(),
            &[
                equivalent,
                collect(
                    Exchange::Reflexivity,
                    ready.into_ref().into(),
                    ready.into_ref().into(),
                    &[],
                ),
            ],
        );

        let selection = storage.add_tree(&[combination.into(), number(2)]);
        let selection_ready = storage.add_tree(&[ready.into(), number(2)]);
        let evaluated = collect(
            Exchange::Eval,
            selection.into(),
            selection_ready.into(),
            &[evaluated, value(number(2))],
        );
        let selected = witness(
            storage,
            storage.selection_certificate(selection_ready).unwrap(),
        );
        let selection_thunk = selection.into_ref().selection();
        let thought = collect(
            Exchange::Think,
            selection_thunk.into(),
            blob.into(),
            &[evaluated, selected],
        );
        let forced = collect(
            Exchange::ThinkToForce,
            selection_thunk.into(),
            blob.into(),
            &[thought],
        );
        collect(
            Exchange::ForceToEncode,
            selection_thunk.strict().into(),
            blob.into(),
            &[forced],
        );
        assert_eq!(
            storage.equals(evaluator.eval(selection_thunk.strict().into()), blob.into()),
            Ok(true)
        );
    }

    #[kernel::test]
    fn collector_composes_authenticated_coupons() {
        let runtime = test_runtime();
        let storage = runtime.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_COLLECTOR"));
        let number = |n: u64| Handle::from(Blob::literal(&n.to_le_bytes()).unwrap());
        let value = number(42);
        let coupon = storage.certificate(0, value, value);
        let claim = storage.certificate_claim(coupon).unwrap();
        let mut witness = storage.add_tree(&[coupon.into(), claim.into()]);
        for _ in 0..32 {
            let witnesses = storage.add_tree(&[witness.into(), witness.into()]);
            let combination = storage.add_tree(&[
                Blob::literal(b"").unwrap().into(),
                program.into(),
                number(fix::coupon::Exchange::Transitivity as u64),
                value,
                value,
                witnesses.into(),
            ]);
            let coupon = Tree::try_from(runtime.execute_privileged(combination).unwrap()).unwrap();
            assert!(coupon.is_tag());
            let fields = storage.get_tree(coupon).unwrap();
            assert!(fields[0].same_encoding(&program.into()));
            let result = storage.certificate_claim(coupon).unwrap();
            let fields = storage.get_tree(result).unwrap();
            assert!(
                fields
                    .iter()
                    .zip([number(0), value, value])
                    .all(|(actual, expected)| actual.same_encoding(&expected))
            );
            witness = storage.add_tree(&[coupon.into(), result.into()]);
        }
    }

    #[kernel::test]
    fn privileged_collector() {
        let runtime = test_runtime();
        let storage = runtime.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_COLLECTOR"));
        let number = |n: u64| Handle::from(Blob::literal(&n.to_le_bytes()).unwrap());
        let left: Handle = storage
            .add_blob(b"left")
            .into_ref()
            .identification()
            .strict()
            .into();
        let middle: Handle = storage.add_tree(&[left]).into();
        let right: Handle = storage
            .add_blob(b"right")
            .into_ref()
            .identification()
            .into();
        let coupon = |author, lhs, rhs| {
            let claim = storage.add_tree(&[number(0), lhs, rhs]);
            let tag = storage
                .add_tree(&[author, claim.into_ref().into()])
                .tag_descriptor();
            storage.add_tree(&[tag.into(), claim.into()]).into()
        };
        let coupons = storage.add_tree(&[
            coupon(program.into(), left, middle),
            coupon(program.into(), middle, right),
        ]);
        let combination = storage.add_tree(&[
            Blob::literal(b"").unwrap().into(),
            program.into(),
            number(fix::coupon::Exchange::Transitivity as u64),
            left,
            right,
            coupons.into(),
        ]);
        let result = Tree::try_from(runtime.execute_privileged(combination).unwrap()).unwrap();
        assert!(result.is_tag());
        let fields = storage.get_tree(result).unwrap();
        let claim = storage
            .get_tree(storage.certificate_claim(result).unwrap())
            .unwrap();
        assert!(claim[0].same_encoding(&number(0)));
        assert!(claim[1].same_encoding(&left));
        assert!(claim[2].same_encoding(&right));
        assert!(fields[0].same_encoding(&program.into()));
        let a = storage.add_blob(b"a");
        let b = storage.add_blob(b"b");
        let xs = storage.add_tree(&[a.into()]);
        let ys = storage.add_tree(&[b.into()]);
        let ta = a.into_ref().identification();
        let tb = b.into_ref().identification();
        for (lhs, rhs, premise_lhs, premise_rhs) in [
            (xs.into(), ys.into(), a.into(), b.into()),
            (
                xs.into_ref().into(),
                ys.into_ref().into(),
                xs.into(),
                ys.into(),
            ),
            (a.into_ref().into(), b.into_ref().into(), a.into(), b.into()),
            (
                xs.into_ref().application().into(),
                ys.into_ref().application().into(),
                xs.into(),
                ys.into(),
            ),
            (
                xs.into_ref().selection().into(),
                ys.into_ref().selection().into(),
                xs.into(),
                ys.into(),
            ),
            (
                ta.into(),
                tb.into(),
                a.into_ref().into(),
                b.into_ref().into(),
            ),
            (ta.strict().into(), tb.strict().into(), ta.into(), tb.into()),
            (
                ta.shallow().into(),
                tb.shallow().into(),
                ta.into(),
                tb.into(),
            ),
        ] {
            let proofs = storage.add_tree(&[coupon(program.into(), premise_lhs, premise_rhs)]);
            let request = storage.add_tree(&[
                Blob::literal(b"").unwrap().into(),
                program.into(),
                number(fix::coupon::Exchange::Congruence as u64),
                lhs,
                rhs,
                proofs.into(),
            ]);
            let output = Tree::try_from(runtime.execute_privileged(request).unwrap()).unwrap();
            let claim = storage
                .get_tree(storage.certificate_claim(output).unwrap())
                .unwrap();
            assert!(claim[0].same_encoding(&number(0)));
            assert_eq!(storage.canonicalize(claim[1]), storage.canonicalize(lhs));
            assert_eq!(storage.canonicalize(claim[2]), storage.canonicalize(rhs));
        }
    }

    #[kernel::test]
    fn storage_identification_preserves_tag_authority() {
        let storage = storage::memory::MemoryStorage::default();
        let authors = storage.add_tree(&[]);
        let fields = [authors.into(), storage.add_blob(b"claim").into()];
        let plain = storage.add_tree(&fields);
        let canonical: Handle = storage.canonicalize(plain.into()).unwrap().into();
        assert!(
            storage
                .identify(canonical.as_bytes())
                .unwrap()
                .same_encoding(&canonical)
        );
        let tagged = Tree::try_from(canonical).unwrap().tag_descriptor();
        assert!(storage.identify(tagged.as_handle().as_bytes()).is_err());
        let genuine = storage.add_tree(&fields).tag_descriptor();
        let genuine: Handle = storage.canonicalize(genuine.into()).unwrap().into();
        assert!(
            storage
                .identify(genuine.as_bytes())
                .unwrap()
                .same_encoding(&genuine)
        );
    }

    fn scheduler_worker_events(mode: u64, workers: usize) -> Result<Vec<u64>, &'static str> {
        let runtime = Arc::new(test_runtime());
        let storage = runtime.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_WORKER_EVENTS"));
        let mode = storage.add_blob(&mode.to_le_bytes());
        let combination = storage.add_tree(&[
            Blob::literal(b"").unwrap().into(),
            program.into(),
            mode.into(),
        ]);
        let report = user_scheduler_driver::start(
            runtime.clone(),
            combination,
            workers,
            user_scheduler_driver::PersistenceOnly(Ok),
        )?;
        let values = storage
            .get_tree(Tree::try_from(report.result.unwrap()).unwrap())
            .unwrap();
        Ok(values
            .iter()
            .map(|value| {
                let bytes = storage.get_blob(Blob::try_from(*value).unwrap()).unwrap();
                u64::from_le_bytes(bytes.as_ref().try_into().unwrap())
            })
            .collect())
    }

    #[kernel::test]
    fn scheduler_worker_notifications_do_not_lose_prior_wakes() {
        assert_eq!(scheduler_worker_events(0, 1), Ok(vec![64]));
    }

    #[kernel::test]
    fn scheduler_worker_notifications_wake_one_and_close_on_finish() {
        assert_eq!(scheduler_worker_events(3, 4), Ok(vec![4656]));
    }

    #[kernel::test]
    fn scheduler_worker_notifications_wake_all_and_close_on_finish() {
        assert_eq!(scheduler_worker_events(1, 4), Ok(vec![96]));
    }

    #[kernel::test]
    fn scheduler_worker_notifications_close_on_failure() {
        assert_eq!(
            scheduler_worker_events(2, 4),
            Err("expected an effect combination")
        );
    }

    #[kernel::test]
    fn userspace_scheduler_certifies_and_persists_evaluation() {
        use fix::arca::trust::Trust;
        let memory = Arc::new(storage::memory::MemoryStorage::default());
        let compiler = Blob::literal(b"test compiler").unwrap();
        let program = |bytes: &[u8]| {
            memory
                .add_tree(&[
                    compiler.into(),
                    memory.add_blob(bytes).into(),
                    Blob::literal(b"Runnable").unwrap().into(),
                ])
                .tag_descriptor()
        };
        let scheduler = program(kernel::testing::fixture("FIX_SCHEDULER"));
        let collector = program(kernel::testing::fixture("FIX_COLLECTOR"));
        let identity = program(stdlib::PRIMITIVES[0].1);
        let mut trust = Trust::parse(&format!("{}\n", compiler.as_handle())).unwrap();
        trust.add_tagger(&*memory, collector).unwrap();
        let runtime = Arc::new(FixOnArca::new(memory.clone()).with_trust(trust));
        let mode: Handle = Blob::literal(b"eval").unwrap().into();
        let zero: Handle = Blob::literal(&0u64.to_le_bytes()).unwrap().into();
        let data = memory.add_blob(&[42; 64]);
        let call = memory.add_tree(&[zero, identity.into(), data.into()]);
        let pending = call.into_ref().application().strict();
        let authors = memory.add_tree(&[]);
        let input = memory
            .add_tree(&[authors.into(), pending.into()])
            .tag_descriptor();
        let mut previous: Option<Handle> = None;
        for workers in [1, 2] {
            let combination =
                memory.add_tree(&[mode, scheduler.into(), collector.into(), input.into()]);
            let saved = Arc::new(core::sync::atomic::AtomicUsize::new(0));
            let written = saved.clone();
            let destination = memory.clone();
            let report = user_scheduler_driver::start(
                runtime.clone(),
                combination,
                workers,
                user_scheduler_driver::PersistenceOnly(move |value: Handle| {
                    if !value.same_encoding(collector.as_handle()) {
                        let coupon = Tree::try_from(value).unwrap();
                        let claim = destination.certificate_claim(coupon).unwrap();
                        let fields = destination.get_tree(claim).unwrap();
                        assert_eq!(fields.len(), 3);
                        assert!(
                            fields[0]
                                .same_encoding(&Blob::literal(&1u64.to_le_bytes()).unwrap().into())
                        );
                        assert!(fields[1].same_encoding(input.as_handle()));
                        written.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                    }
                    destination
                        .canonicalize(value)
                        .map(Into::into)
                        .map_err(|_| "save failed")
                }),
            )
            .unwrap();
            if let Some(previous) = previous {
                assert!(report.result.unwrap().same_encoding(&previous));
            }
            previous = Some(report.result.unwrap());
            let claim = memory
                .certificate_claim(Tree::try_from(report.result.unwrap()).unwrap())
                .unwrap();
            let fields = memory.get_tree(claim).unwrap();
            assert!(fields[0].same_encoding(&Blob::literal(&1u64.to_le_bytes()).unwrap().into()));
            assert!(fields[1].same_encoding(input.as_handle()));
            let expected = memory
                .add_tree(&[authors.into(), call.into()])
                .tag_descriptor();
            assert_eq!(memory.equals(fields[2], expected.into()), Ok(true));
            assert!(saved.load(core::sync::atomic::Ordering::Relaxed) > 0);
        }
        let mut requests = Vec::new();
        let mut expected = Vec::new();
        for index in 0..8u64 {
            let call = memory.add_tree(&[
                zero,
                identity.into(),
                memory.add_blob(&index.to_le_bytes()).into(),
            ]);
            let thunk = call.into_ref().application();
            for returned in [
                Handle::from(thunk),
                thunk.strict().into(),
                thunk.shallow().into(),
            ] {
                let container = memory.add_tree(&[returned]);
                let selection = memory
                    .add_tree(&[container.into_ref().into(), zero])
                    .into_ref()
                    .selection();
                requests.push(selection.strict().into());
                expected.push(call.into());
            }
        }
        let requests = memory.add_tree(&requests);
        let combination =
            memory.add_tree(&[mode, scheduler.into(), collector.into(), requests.into()]);
        let destination = memory.clone();
        let report = user_scheduler_driver::start(
            runtime,
            combination,
            2,
            user_scheduler_driver::PersistenceOnly(move |value| {
                destination
                    .canonicalize(value)
                    .map(Into::into)
                    .map_err(|_| "save failed")
            }),
        )
        .unwrap();
        let claim = memory
            .certificate_claim(Tree::try_from(report.result.unwrap()).unwrap())
            .unwrap();
        let claim = memory.get_tree(claim).unwrap();
        assert_eq!(
            memory.equals(claim[2], memory.add_tree(&expected).into()),
            Ok(true)
        );
    }

    #[kernel::test]
    fn collector_reconstructs_evaluated_tags() {
        use fix::arca::trust::Trust;
        use fix::coupon::Exchange::*;
        let runtime = test_runtime();
        let program = test_program(runtime.storage(), kernel::testing::fixture("FIX_COLLECTOR"));
        let compiler = Blob::canonical(stdlib::PRIMITIVES[0].1)
            .unwrap()
            .object()
            .into_ref();
        let mut trust = Trust::parse(&format!("{}\n", compiler.as_handle())).unwrap();
        trust.add_tagger(runtime.storage(), program).unwrap();
        let runtime = runtime.with_trust(trust);
        let storage = runtime.storage();
        let number = |n: u64| Handle::from(Blob::literal(&n.to_le_bytes()).unwrap());
        let metadata = Blob::literal(b"").unwrap().into();
        let request = |rule, lhs, rhs, witnesses: &[Handle]| {
            let witnesses = storage.add_tree(witnesses);
            runtime
                .execute_privileged(storage.add_tree(&[
                    metadata,
                    program.into(),
                    number(rule),
                    lhs,
                    rhs,
                    witnesses.into(),
                ]))
                .unwrap()
        };
        let issue = |rule: fix::coupon::Exchange, lhs, rhs, witnesses: &[Handle]| {
            let coupon = Tree::try_from(request(rule as u64, lhs, rhs, witnesses)).unwrap();
            storage
                .add_tree(&[
                    coupon.into(),
                    storage.certificate_claim(coupon).unwrap().into(),
                ])
                .into()
        };
        let blob = storage.add_blob(&[42; 64]);
        let reference: Handle = blob.into_ref().into();
        let thunk: Handle = blob.into_ref().identification().into();
        let encoded: Handle = blob.into_ref().identification().strict().into();
        let thought = issue(Think, thunk, reference, &[]);
        let forced = issue(ThinkToForce, thunk, reference, &[thought]);
        let equivalent = issue(ForceToEncode, encoded, blob.into(), &[forced]);
        let reversed = issue(Symmetry, blob.into(), encoded, &[equivalent]);
        let leaf = issue(Eval, blob.into(), blob.into(), &[]);
        let payload = issue(EvalEq, encoded, blob.into(), &[leaf, reversed]);
        for names in [vec![], vec![number(100)], vec![number(100), number(100)]] {
            let authors = storage.add_tree(&names);
            let author_witnesses: Vec<_> = names
                .iter()
                .map(|name| issue(Eval, *name, *name, &[]))
                .collect();
            let author_proof = issue(Eval, authors.into(), authors.into(), &author_witnesses);
            let original = storage
                .add_tree(&[authors.into(), encoded])
                .tag_descriptor();
            let fields = storage.add_tree(&[authors.into(), blob.into()]);
            let witnesses = [author_proof, payload];
            let coupon = request(Eval as u64, original.into(), fields.into(), &witnesses);
            let primitive = storage.certificate(1, authors.into(), authors.into());
            let primitive_witness = storage.add_tree(&[
                primitive.into(),
                storage.certificate_claim(primitive).unwrap().into(),
            ]);
            let mixed_coupon = request(
                Eval as u64,
                original.into(),
                fields.into(),
                &[primitive_witness.into(), payload],
            );
            let claim = storage
                .certificate_claim(Tree::try_from(coupon).unwrap())
                .unwrap();
            let claim = storage.get_tree(claim).unwrap();
            let mixed = storage
                .certificate_claim(Tree::try_from(mixed_coupon).unwrap())
                .unwrap();
            let mixed = storage.get_tree(mixed).unwrap();
            assert!(claim[0].same_encoding(&number(1)));
            assert_eq!(
                storage.canonicalize(claim[1]),
                storage.canonicalize(original.into())
            );
            assert_eq!(storage.equals(claim[2], mixed[2]), Ok(true));
            assert!(Tree::try_from(claim[2]).unwrap().is_tag());
            assert_eq!(
                storage.equals(claim[2], fields.tag_descriptor().into()),
                Ok(true)
            );
        }
    }

    #[kernel::test]
    fn application_coupons() {
        let runtime = test_runtime();
        let storage = runtime.storage();
        let program = test_program(storage, stdlib::PRIMITIVES[0].1);
        let number = |n: u64| Handle::from(Blob::literal(&n.to_le_bytes()).unwrap());
        let pending = storage.add_tree(&[program.into_ref().application().strict().into()]);
        let arguments = storage.add_tree(&[
            pending.into_ref().into(),
            pending.into_ref().application().into(),
        ]);
        let combination = storage.add_tree(&[
            Blob::literal(b"").unwrap().into(),
            program.into(),
            arguments.tag_descriptor().into(),
        ]);
        assert!(!combination.as_handle().is_comparable());
        let coupon = runtime.execute(combination).unwrap();
        let fields = storage.get_tree(coupon).unwrap();
        assert!(coupon.is_tag());
        assert_eq!(fields.len(), 2);
        assert!(
            storage
                .get_tree(Tree::try_from(fields[0]).unwrap())
                .unwrap()
                .is_empty()
        );
        let claim = storage.certificate_claim(coupon).unwrap();
        let assertion = storage.get_tree(claim).unwrap();
        assert!(assertion[0].same_encoding(&number(2)));
        assert!(assertion[1].same_encoding(&combination.into()));
        assert!(assertion[2].same_encoding(&combination.into()));
    }

    #[kernel::test]
    fn canonicalization_certificates() {
        let runtime = test_runtime();
        let storage = runtime.storage();
        let blob = storage.add_blob(&[0xff; 64]);
        let tree = storage.add_tree(&[
            blob.into(),
            blob.into_ref().identification().strict().into(),
        ]);
        let number = |n: u64| Handle::from(Blob::literal(&n.to_le_bytes()).unwrap());
        for value in [
            Handle::from(blob),
            blob.into_ref().into(),
            tree.tag_descriptor().into(),
            tree.into_ref().into(),
            tree.into_ref().application().into(),
            tree.into_ref().selection().shallow().into(),
        ] {
            let certificate = storage.canonicalization_certificate(value).unwrap();
            assert!(certificate.is_tag());
            let fields = storage.get_tree(certificate).unwrap();
            assert_eq!(fields.len(), 2);
            assert!(
                storage
                    .get_tree(Tree::try_from(fields[0]).unwrap())
                    .unwrap()
                    .is_empty()
            );
            let claim = storage.certificate_claim(certificate).unwrap();
            let assertion = storage.get_tree(claim).unwrap();
            assert!(assertion[0].same_encoding(&number(0)));
            assert!(assertion[1].same_encoding(&value));
            let canonical = assertion[2];
            assert!(canonical.is_canonical());
            assert_eq!(canonical.as_bytes()[31], value.as_bytes()[31]);
            assert_eq!(canonical.is_comparable(), value.is_comparable());
        }
    }

    #[kernel::test]
    fn privileged_handle_bytes() {
        let runtime = test_runtime();
        let storage = runtime.storage();
        let elf = kernel::testing::fixture("FIX_PRIVILEGED");
        let program = test_program(storage, elf);
        let combination = storage.add_tree(&[Blob::literal(b"").unwrap().into(), program.into()]);
        assert!(!combination.as_handle().is_canonical());
        let pair = storage
            .get_tree(Tree::try_from(runtime.execute_privileged(combination).unwrap()).unwrap())
            .unwrap();
        assert_eq!(
            storage
                .get_blob(Blob::try_from(pair[0]).unwrap())
                .unwrap()
                .as_ref(),
            combination.as_handle().as_bytes()
        );
        let thunk = program.object().into_ref().identification();
        for value in [
            program.into(),
            program.into_ref().into(),
            thunk.into(),
            thunk.strict().into(),
        ] {
            let application =
                storage.add_tree(&[Blob::literal(b"").unwrap().into(), pair[1], value]);
            let result = runtime.execute_privileged(application).unwrap();
            assert_eq!(
                storage
                    .get_blob(Blob::try_from(result).unwrap())
                    .unwrap()
                    .as_ref(),
                value.as_bytes()
            );
        }

        let function = common::elfloader::load_elf(elf).unwrap();
        let argument = kernel::prelude::Blob::new(0u64.to_le_bytes());
        let effect: kernel::prelude::Function =
            function.apply(argument).force().try_into().unwrap();
        let description: kernel::prelude::Tuple = effect.into_inner().read().try_into().unwrap();
        let name: kernel::prelude::Blob = description.get(1).try_into().unwrap();
        assert_eq!(&*name, b"is_privileged");
        let mut arguments: kernel::prelude::Tuple = description.get(2).try_into().unwrap();
        let continuation: kernel::prelude::Function = arguments.take(0).try_into().unwrap();
        let trapped: kernel::prelude::Function = continuation
            .apply(kernel::prelude::Word::new(0))
            .force()
            .try_into()
            .unwrap();
        let description: kernel::prelude::Tuple = trapped.into_inner().read().try_into().unwrap();
        let arguments: kernel::prelude::Tuple = description.get(2).try_into().unwrap();
        let reason: kernel::prelude::Blob = arguments.get(0).try_into().unwrap();
        assert_eq!(&*reason, b"exception");
    }

    #[kernel::test]
    fn application_preserves_indistinguishability() {
        fn assert_indistinguishable(evaluator: &Evaluation, left: Handle, right: Handle) {
            if left.same_encoding(&right) {
                return;
            }
            assert_eq!(left.is_comparable(), right.is_comparable());
            let storage = evaluator.storage();
            match (left.view(), right.view()) {
                (HandleView::Object(left), HandleView::Object(right)) => {
                    match (left.view(), right.view()) {
                        (ObjectView::Blob(left), ObjectView::Blob(right)) => {
                            assert_eq!(
                                storage.equals((*left).into(), (*right).into()),
                                Ok(true),
                                "indistinguishable inputs produced distinguishable Blob contents"
                            );
                        }
                        (ObjectView::Tree(left), ObjectView::Tree(right)) => {
                            assert_eq!(left.is_tag(), right.is_tag());
                            assert_eq!(left.len(), right.len());
                            let left = storage.get_tree(*left).unwrap();
                            let right = storage.get_tree(*right).unwrap();
                            for (left, right) in left.iter().zip(right.iter()) {
                                assert_indistinguishable(evaluator, *left, *right);
                            }
                        }
                        _ => panic!("different Object types"),
                    }
                }
                (HandleView::Ref(left), HandleView::Ref(right)) => {
                    assert_indistinguishable(
                        evaluator,
                        left.object_descriptor().into(),
                        right.object_descriptor().into(),
                    );
                }
                (HandleView::Thunk(left), HandleView::Thunk(right)) => {
                    assert_indistinguishable(
                        evaluator,
                        evaluator.think(*left),
                        evaluator.think(*right),
                    );
                }
                (HandleView::Encode(left_encode), HandleView::Encode(right_encode)) => {
                    assert!(matches!(
                        (left_encode.view(), right_encode.view()),
                        (EncodeView::Strict(_), EncodeView::Strict(_))
                            | (EncodeView::Shallow(_), EncodeView::Shallow(_))
                    ));
                    assert_indistinguishable(
                        evaluator,
                        evaluator.eval(left),
                        evaluator.eval(right),
                    );
                }
                _ => panic!("different value types"),
            }
        }

        let evaluator = Evaluation::new(test_runtime());
        let storage = evaluator.storage();
        let program = test_program(
            storage,
            kernel::testing::fixture("FIX_INDISTINGUISHABILITY"),
        );
        let metadata: Handle = Blob::literal(b"").unwrap().into();
        let value: Handle = storage.add_blob(&[42; 64]).into();
        let short = storage.add_tree(&[metadata, program.into(), value]);
        let long = storage.add_tree(&[metadata, program.into(), value, metadata]);
        let request = storage.add_tree(&[storage.add_blob(b"test").into()]);
        let target = storage.add_tree(&[value]);
        let selection =
            storage.add_tree(&[target.into(), storage.add_blob(&0u64.to_le_bytes()).into()]);
        let thunks = [
            short.into_ref().application(),
            long.into_ref().application(),
            selection.into_ref().selection(),
        ];
        let inputs = thunks.map(|thunk| {
            assert_eq!(storage.equals(evaluator.think(thunk), value), Ok(true));
            storage.add_tree(&[metadata, program.into(), thunk.into(), request.into()])
        });
        for input in &inputs {
            assert_indistinguishable(&evaluator, inputs[0].into(), (*input).into());
        }

        let results = inputs.map(|input| apply(evaluator.runtime(), input));
        for result in &results {
            assert_indistinguishable(&evaluator, results[0], *result);
        }
    }

    #[kernel::test]
    fn eval_resumes_with_evaluated_argument() {
        let evaluator = Evaluation::new(test_runtime());
        let storage = evaluator.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_EVAL"));
        let input = Blob::literal(&41u64.to_le_bytes()).unwrap();
        for encoded in [false, true] {
            let argument = if encoded {
                input.into_ref().identification().strict().into()
            } else {
                input.into()
            };
            let combination = storage.add_tree(&[
                Blob::literal(b"").unwrap().into(),
                program.into(),
                input.into(),
                Blob::literal(&(encoded as u64).to_le_bytes())
                    .unwrap()
                    .into(),
            ]);
            let result = apply(evaluator.runtime(), combination);
            let thunk = Thunk::try_from(result).unwrap();
            let ThunkView::Application(reference) = thunk.view() else {
                panic!("expected continuation application")
            };
            let fields = storage.get_tree(reference.object_descriptor()).unwrap();
            assert_eq!(fields.len(), 3);
            assert!(fields[2].same_encoding(&argument));
            for canonical in [false, true] {
                let thunk = if canonical {
                    Thunk::try_from(Handle::from(storage.canonicalize(thunk.into()).unwrap()))
                        .unwrap()
                } else {
                    thunk
                };
                let output = evaluator.eval(thunk.strict().into());
                assert_eq!(
                    storage
                        .get_blob(Blob::try_from(output).unwrap())
                        .unwrap()
                        .as_ref(),
                    &42u64.to_le_bytes()
                );
            }
        }
    }

    #[kernel::test]
    fn sdk_attachments_survive_reclamation_and_continuation_replay() {
        let runtime = test_runtime();
        let storage = runtime.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_SDK_ATTACHMENTS"));
        let empty: Handle = Blob::literal(b"").unwrap().into();
        let input = storage.add_tree(&[
            Blob::literal(&20u64.to_le_bytes()).unwrap().into(),
            Blob::literal(&22u64.to_le_bytes()).unwrap().into(),
        ]);
        let combination = storage.add_tree(&[empty, program.into(), input.into()]);
        let request = Tree::try_from(apply(&runtime, combination)).unwrap();
        let continuation = storage.get_tree(request).unwrap()[1];
        let canonical = storage.canonicalize(continuation).unwrap().into();
        for continuation in [continuation, continuation, canonical] {
            let combination = storage.add_tree(&[empty, continuation, empty]);
            let result = Blob::try_from(apply(&runtime, combination)).unwrap();
            assert_eq!(
                storage.get_blob(result).unwrap().as_ref(),
                &42u64.to_le_bytes()
            );
        }
    }

    #[kernel::test]
    fn blob_reads_preserve_slices_and_snapshots() {
        let runtime = test_runtime();
        let storage = runtime.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_READ_BLOB"));
        let bytes: Vec<u8> = (0..8193).map(|index| (index % 251) as u8).collect();
        let machine: Handle = storage.add_blob(&bytes).into();
        let canonical: Handle = storage.canonicalize(machine).unwrap().into();
        let empty: Handle = Blob::literal(b"").unwrap().into();
        let mut expected = None;
        for input in [machine, canonical] {
            let combination = storage.add_tree(&[empty, program.into(), input]);
            let request = Tree::try_from(apply(&runtime, combination)).unwrap();
            let continuation = storage.get_tree(request).unwrap()[1];
            let canonical = storage.canonicalize(continuation).unwrap().into();
            for continuation in [continuation, continuation, canonical] {
                let combination = storage.add_tree(&[empty, continuation, empty]);
                let result = Blob::try_from(apply(&runtime, combination)).unwrap();
                let bytes = storage.get_blob(result).unwrap();
                assert_eq!(bytes.len(), 131072);
                if let Some(expected) = &expected {
                    assert_eq!(bytes.as_ref(), expected);
                } else {
                    expected = Some(bytes.to_vec());
                }
            }
        }
    }

    #[kernel::test]
    fn trusted_taggers_preserve_fields_and_replay_authority() {
        use fix::arca::trust::Trust;
        let runtime = test_runtime();
        let program = test_program(runtime.storage(), kernel::testing::fixture("FIX_TAGGER"));
        let author = Blob::canonical(stdlib::PRIMITIVES[0].1)
            .unwrap()
            .object()
            .into_ref();
        let mut trust = Trust::parse(&format!("{}\n", author.as_handle())).unwrap();
        trust.add_tagger(runtime.storage(), program).unwrap();
        let runtime = runtime.with_trust(trust);
        let storage = runtime.storage();
        let empty: Handle = Blob::literal(b"").unwrap().into();
        for names in [vec![], vec![empty], vec![empty, empty]] {
            let authors = storage.add_tree(&names);
            let fields = storage.add_tree(&[authors.into(), storage.add_blob(&[42; 64]).into()]);
            let combination = |pause: u64| {
                storage.add_tree(&[
                    empty,
                    program.into(),
                    fields.into(),
                    storage.add_blob(&pause.to_le_bytes()).into(),
                    empty,
                ])
            };
            let result = runtime.execute_privileged(combination(0)).unwrap();
            let expected = fields.tag_descriptor();
            assert_eq!(storage.equals(result, expected.into()), Ok(true));
            assert!(
                storage
                    .get_tree(Tree::try_from(result).unwrap())
                    .unwrap()
                    .iter()
                    .zip(storage.get_tree(fields).unwrap().iter())
                    .all(|(a, b)| a.same_encoding(b))
            );
            let request = runtime.execute_privileged(combination(1)).unwrap();
            let pair = storage.get_tree(Tree::try_from(request).unwrap()).unwrap();
            let continuation = pair[1];
            for persisted in [false, true] {
                let continuation = if persisted {
                    storage.canonicalize(continuation).unwrap().into()
                } else {
                    continuation
                };
                let call = storage.add_tree(&[empty, continuation, empty]);
                let result = runtime.execute_privileged(call).unwrap();
                assert_eq!(storage.equals(result, expected.into()), Ok(true));
                let fields = storage.get_tree(Tree::try_from(result).unwrap()).unwrap();
                assert_eq!(
                    storage
                        .get_tree(Tree::try_from(fields[0]).unwrap())
                        .unwrap()
                        .len(),
                    names.len()
                );
            }
        }
    }

    #[kernel::test]
    fn sdk_effects_preserve_owned_captures() {
        use fix::arca::EffectAction;
        let runtime = test_runtime();
        let storage = runtime.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_SDK_EVAL"));
        let argument = storage.add_blob(&[42; 64]);
        let combination = storage.add_tree(&[
            Blob::literal(b"").unwrap().into(),
            program.into(),
            argument.into(),
            program.into(),
            storage.add_blob(&4u64.to_le_bytes()).into(),
            storage.add_blob(&8u64.to_le_bytes()).into(),
        ]);
        let mut calls = 0;
        let result = runtime
            .execute_with_effects(combination, true, &mut |effect| {
                let fields = storage.get_tree(Tree::try_from(effect).unwrap()).unwrap();
                assert_eq!(
                    storage
                        .get_blob(Blob::try_from(fields[0]).unwrap())
                        .unwrap()
                        .as_ref(),
                    b"sdk_test"
                );
                assert!(fields[1].same_encoding(&argument.into()));
                calls += 1;
                EffectAction::Resume(argument.into())
            })
            .unwrap();
        assert_eq!(calls, 8);
        assert_eq!(
            storage
                .get_blob(Blob::try_from(result).unwrap())
                .unwrap()
                .as_ref(),
            argument.as_handle().as_bytes()
        );
    }

    #[kernel::test]
    fn sdk_eval_preserves_owned_captures() {
        let evaluator = Evaluation::new(test_runtime());
        let storage = evaluator.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_SDK_EVAL"));
        let identity = test_program(storage, stdlib::PRIMITIVES[0].1);
        let argument: Handle = storage.add_blob(&[42; 64]).into();
        for mode in 0u64..4 {
            let combination = storage.add_tree(&[
                Blob::literal(b"").unwrap().into(),
                program.into(),
                argument,
                identity.into(),
                Blob::literal(&mode.to_le_bytes()).unwrap().into(),
                Blob::literal(&4u64.to_le_bytes()).unwrap().into(),
            ]);
            let result = evaluator.eval(combination.into_ref().application().strict().into());
            assert_eq!(storage.equals(result, argument), Ok(true));
        }
    }

    #[kernel::test]
    fn determinism_across_sdk_eval_and_capture_reuse() {
        let evaluator = Evaluation::new(test_runtime());
        let storage = evaluator.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_SDK_EVAL"));
        let identity = test_program(storage, stdlib::PRIMITIVES[0].1);
        let metadata: Handle = Blob::literal(b"").unwrap().into();
        let argument: Handle = storage.add_blob(&[42; 64]).into();
        let combination = storage.add_tree(&[
            metadata,
            program.into(),
            argument,
            identity.into(),
            Blob::literal(&3u64.to_le_bytes()).unwrap().into(),
            Blob::literal(&2u64.to_le_bytes()).unwrap().into(),
        ]);
        let canonical = Tree::try_from(Handle::from(
            storage.canonicalize(combination.into()).unwrap(),
        ))
        .unwrap();
        let mut runs = Vec::new();
        for combination in [combination, canonical] {
            let mut result = apply(evaluator.runtime(), combination);
            let mut checkpoints = Vec::new();
            let mut replay = None;
            while let Ok(thunk) = Thunk::try_from(result) {
                let ThunkView::Application(request) = thunk.view() else {
                    panic!("expected eval continuation")
                };
                let request = storage.get_tree(request.object_descriptor()).unwrap();
                if replay.is_none() {
                    replay = Some(storage.canonicalize(result).unwrap());
                }
                let checkpoint = storage.canonicalize(request[1]).unwrap();
                let value = evaluator.eval(request[2]);
                let resumed = storage.add_tree(&[metadata, checkpoint.into(), value]);
                result = apply(evaluator.runtime(), resumed);
                checkpoints.push(checkpoint);
            }
            assert_eq!(storage.equals(result, argument), Ok(true));
            let replay = Thunk::try_from(Handle::from(replay.unwrap())).unwrap();
            assert_eq!(
                storage.equals(evaluator.eval(replay.strict().into()), argument),
                Ok(true)
            );
            runs.push(checkpoints);
        }
        assert_eq!(runs[0], runs[1]);
    }

    #[kernel::test]
    fn recursive_local_graph_preserves_aliases() {
        let evaluator = Evaluation::new(test_runtime());
        let storage = evaluator.storage();
        let program = test_program(
            storage,
            kernel::testing::fixture("FIX_RECURSIVE_LOCAL_ALIASES"),
        );
        let combination = storage.add_tree(&[Blob::literal(b"").unwrap().into(), program.into()]);
        let result = apply(evaluator.runtime(), combination);
        let fields = storage.get_tree(Tree::try_from(result).unwrap()).unwrap();
        assert_eq!(fields.len(), 5);
        assert!(fields[0].same_encoding(&fields[1]));
        assert!(fields[2].same_encoding(&fields[3]));
        assert_eq!(storage.equals(fields[0], fields[1]), Ok(true));
        assert_eq!(storage.equals(fields[2], fields[3]), Ok(true));
        let bytes = storage
            .get_blob(Blob::try_from(fields[0]).unwrap())
            .unwrap();
        assert_eq!(bytes.as_ref(), &[0x29; 64]);
        let pair = storage
            .get_tree(Tree::try_from(fields[2]).unwrap())
            .unwrap();
        assert_eq!(pair.len(), 2);
        for field in pair.iter() {
            assert!(field.same_encoding(&fields[0]));
            assert_eq!(storage.equals(*field, fields[0]), Ok(true));
        }
        let nested = storage
            .get_tree(Tree::try_from(fields[4]).unwrap())
            .unwrap();
        assert_eq!(nested.len(), 4);
        for (actual, expected) in nested.iter().zip(fields.iter()) {
            assert!(actual.same_encoding(expected));
            assert_eq!(storage.equals(*actual, *expected), Ok(true));
        }
    }

    #[kernel::test]
    fn large_local_graph_preserves_each_blob() {
        let evaluator = Evaluation::new(test_runtime());
        let storage = evaluator.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_LARGE_LOCAL_GRAPH"));
        let combination = storage.add_tree(&[Blob::literal(b"").unwrap().into(), program.into()]);
        let result = apply(evaluator.runtime(), combination);
        let fields = storage.get_tree(Tree::try_from(result).unwrap()).unwrap();
        assert_eq!(fields.len(), 5000);
        for (index, field) in fields.iter().enumerate() {
            let bytes = storage.get_blob(Blob::try_from(*field).unwrap()).unwrap();
            let mut expected = [0u8; 64];
            expected[..4].copy_from_slice(&(index as u32).to_le_bytes());
            assert_eq!(bytes.as_ref(), &expected);
        }
    }

    #[kernel::test]
    fn kernel_effects_preserve_outgoing_values() {
        selective_lifting(false);
    }

    #[kernel::test]
    fn capture_preserves_partially_lifted_graphs() {
        selective_lifting(true);
    }

    fn selective_lifting(escape: bool) {
        use fix::arca::EffectAction;
        use fix::storage::memory::MemoryStorage;

        let memory = Arc::new(MemoryStorage::default());
        let author = Blob::canonical(stdlib::PRIMITIVES[0].1)
            .unwrap()
            .object()
            .into_ref();
        let runtime = FixOnArca::new(memory.clone()).with_trust(
            fix::arca::trust::Trust::parse(&format!("{}\n", author.as_handle())).unwrap(),
        );
        let storage = runtime.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_SELECTIVE_LIFTING"));
        let combination = storage.add_tree(&[Blob::literal(b"").unwrap().into(), program.into()]);
        let mut count = 0;
        let mut leaf = None;
        let result = runtime
            .execute_with_effects(combination, false, &mut |effect| {
                if count == 0 {
                    let fields = storage.get_tree(Tree::try_from(effect).unwrap()).unwrap();
                    let blob = Blob::try_from(fields[0]).unwrap();
                    assert_eq!(storage.get_blob(blob).unwrap().as_ref(), &[7; 64]);
                    assert!(fields[1].same_encoding(&blob.into_ref().into()));
                    assert!(
                        storage
                            .get_tree(Tree::try_from(fields[2]).unwrap())
                            .unwrap()
                            .is_empty()
                    );
                    assert!(
                        fields[3].same_encoding(&blob.into_ref().identification().strict().into())
                    );
                    leaf = Some(blob);
                } else {
                    assert!(effect.same_encoding(&leaf.unwrap().into_ref().into()));
                }
                count += 1;
                if escape && count == 2 {
                    EffectAction::Unhandled
                } else {
                    EffectAction::Resume(Blob::literal(b"").unwrap().into())
                }
            })
            .unwrap();
        assert_eq!(count, 2);
        let results = if escape {
            let pair = storage.get_tree(Tree::try_from(result).unwrap()).unwrap();
            let canonical: Handle = storage.canonicalize(pair[1]).unwrap().into();
            [pair[1], canonical]
                .map(|continuation| {
                    let combination = storage.add_tree(&[
                        Blob::literal(b"").unwrap().into(),
                        continuation,
                        Blob::literal(b"").unwrap().into(),
                    ]);
                    runtime
                        .execute_with_effects(combination, false, &mut |_| {
                            panic!("unexpected effect")
                        })
                        .unwrap()
                })
                .to_vec()
        } else {
            vec![result]
        };
        for result in results {
            let fields = storage.get_tree(Tree::try_from(result).unwrap()).unwrap();
            assert_eq!(fields.len(), 3);
            let hidden = storage
                .get_tree(Tree::try_from(fields[0]).unwrap())
                .unwrap();
            assert!(hidden[0].same_encoding(&fields[1]));
            assert!(hidden[1].same_encoding(&fields[2]));
            assert_eq!(
                storage
                    .get_blob(Blob::try_from(fields[1]).unwrap())
                    .unwrap()
                    .as_ref(),
                &[9; 128]
            );
            let request = storage
                .get_tree(Tree::try_from(fields[2]).unwrap())
                .unwrap();
            assert!(storage.equals(request[0], leaf.unwrap().into()).unwrap());
        }
    }

    #[kernel::test]
    fn kernel_effects_resume_replay_and_stop() {
        use fix::arca::EffectAction;
        use fix::storage::memory::MemoryStorage;

        let memory = Arc::new(MemoryStorage::default());
        let author = Blob::canonical(stdlib::PRIMITIVES[0].1)
            .unwrap()
            .object()
            .into_ref();
        let runtime = FixOnArca::new(memory.clone()).with_trust(
            fix::arca::trust::Trust::parse(&format!("{}\n", author.as_handle())).unwrap(),
        );
        let storage = runtime.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_LINEAR_EFFECTS"));
        let request = storage.add_tree(&[Blob::literal(b"ping").unwrap().into()]);
        let combination = |mode: u64| {
            storage.add_tree(&[
                Blob::literal(b"").unwrap().into(),
                program.into(),
                request.into(),
                Blob::literal(&mode.to_le_bytes()).unwrap().into(),
            ])
        };
        for mode in [0, 3] {
            for privileged in [false, true] {
                let combination = combination(mode);
                let mut count = 0u64;
                let result = runtime
                    .execute_with_effects(combination, privileged, &mut |effect| {
                        assert!(effect.same_encoding(&request.into()));
                        let reply = Blob::literal(&count.to_le_bytes()).unwrap();
                        count += 1;
                        EffectAction::Resume(reply.into())
                    })
                    .unwrap();
                assert_eq!(count, 64);
                assert_eq!(
                    storage
                        .get_blob(Blob::try_from(result).unwrap())
                        .unwrap()
                        .as_ref(),
                    &64u64.to_le_bytes()
                );
            }
        }
        let mut count = 0u64;
        let bytes = runtime
            .execute_with_effects(combination(1), true, &mut |_| {
                let reply = Blob::literal(&count.to_le_bytes()).unwrap();
                count += 1;
                EffectAction::Resume(reply.into())
            })
            .unwrap();
        assert_eq!(count, 64);
        assert_eq!(
            storage
                .get_blob(Blob::try_from(bytes).unwrap())
                .unwrap()
                .as_ref(),
            request.as_handle().as_bytes()
        );

        for mode in [2, 4] {
            let original = combination(mode);
            let mut count = 0u64;
            let result = runtime
                .execute_with_effects(original, false, &mut |_| {
                    if count == 8 {
                        return EffectAction::Unhandled;
                    }
                    let reply = Blob::literal(&count.to_le_bytes()).unwrap();
                    count += 1;
                    EffectAction::Resume(reply.into())
                })
                .unwrap();
            let pair = storage.get_tree(Tree::try_from(result).unwrap()).unwrap();
            assert!(pair[0].same_encoding(&request.into()));
            for canonical in [false, true] {
                let continuation = if canonical {
                    storage.canonicalize(pair[1]).unwrap().into()
                } else {
                    pair[1]
                };
                let resumed = storage.add_tree(&[
                    Blob::literal(b"").unwrap().into(),
                    continuation,
                    Blob::literal(&8u64.to_le_bytes()).unwrap().into(),
                ]);
                let mut count = 9u64;
                let tag = runtime
                    .execute_with_effects(resumed, false, &mut |_| {
                        let reply = Blob::literal(&count.to_le_bytes()).unwrap();
                        count += 1;
                        EffectAction::Resume(reply.into())
                    })
                    .unwrap();
                assert_eq!(count, 64);
                let fields = storage.get_tree(Tree::try_from(tag).unwrap()).unwrap();
                assert_eq!(
                    storage.canonicalize(fields[0]).unwrap(),
                    storage.canonicalize(original.into()).unwrap()
                );
            }
        }
        let stopped = runtime
            .execute_with_effects(combination(0), false, &mut |_| {
                EffectAction::Stop(Blob::literal(b"stopped").unwrap().into())
            })
            .unwrap();
        assert_eq!(
            storage
                .get_blob(Blob::try_from(stopped).unwrap())
                .unwrap()
                .as_ref(),
            b"stopped"
        );
    }

    #[kernel::test]
    fn continuations_resume_across_runtime_instances() {
        use fix::storage::memory::MemoryStorage;

        let memory = Arc::new(MemoryStorage::default());
        let author = Blob::canonical(stdlib::PRIMITIVES[0].1)
            .unwrap()
            .object()
            .into_ref();
        let runtime = FixOnArca::new(memory.clone()).with_trust(
            fix::arca::trust::Trust::parse(&format!("{}\n", author.as_handle())).unwrap(),
        );
        let storage = runtime.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_CONTINUATION"));
        let combination = storage.add_tree(&[
            Blob::literal(b"").unwrap().into(),
            program.into(),
            Blob::literal(&0u64.to_le_bytes()).unwrap().into(),
        ]);
        let request = Tree::try_from(runtime.execute_privileged(combination).unwrap()).unwrap();
        let fields = storage.get_tree(request).unwrap();
        let resumed = storage.add_tree(&[
            Blob::literal(b"").unwrap().into(),
            fields[1],
            Blob::literal(&41u64.to_le_bytes()).unwrap().into(),
        ]);
        let result = Blob::try_from(runtime.execute_privileged(resumed).unwrap()).unwrap();
        assert_eq!(
            storage.get_blob(result).unwrap().as_ref(),
            &42u64.to_le_bytes()
        );
        drop(runtime);
        let runtime = FixOnArca::new(memory.clone());
        let resumed = memory.add_tree(&[
            Blob::literal(b"").unwrap().into(),
            fields[1],
            Blob::literal(&7u64.to_le_bytes()).unwrap().into(),
        ]);
        let result = Blob::try_from(runtime.execute_privileged(resumed).unwrap()).unwrap();
        assert_eq!(
            memory.get_blob(result).unwrap().as_ref(),
            &8u64.to_le_bytes()
        );
    }

    #[kernel::test]
    fn continuations() {
        let evaluator = Evaluation::new(test_runtime());
        let storage = evaluator.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_CONTINUATION"));
        let capture = storage.add_tree(&[
            Blob::literal(b"").unwrap().into(),
            program.into(),
            Blob::literal(&0u64.to_le_bytes()).unwrap().into(),
        ]);
        let request = Tree::try_from(apply(evaluator.runtime(), capture)).unwrap();
        let fields = storage.get_tree(request).unwrap();
        assert_eq!(
            storage
                .get_blob(Blob::try_from(fields[0]).unwrap())
                .unwrap()
                .as_ref(),
            &123u64.to_le_bytes()
        );
        let continuation = fields[1];
        for reply in [41u64, 7] {
            let application = storage.add_tree(&[
                Blob::literal(b"").unwrap().into(),
                continuation,
                Blob::literal(&reply.to_le_bytes()).unwrap().into(),
            ]);
            let result = apply(evaluator.runtime(), application);
            assert_eq!(
                storage
                    .get_blob(Blob::try_from(result).unwrap())
                    .unwrap()
                    .as_ref(),
                &(reply + 1).to_le_bytes()
            );
        }
        for mode in 3u64..=5 {
            let tag_capture = storage.add_tree(&[
                Blob::literal(b"").unwrap().into(),
                program.into(),
                Blob::literal(&mode.to_le_bytes()).unwrap().into(),
            ]);
            let pair = storage
                .get_tree(Tree::try_from(apply(evaluator.runtime(), tag_capture)).unwrap())
                .unwrap();
            for canonical in [false, true] {
                let continuation = if canonical {
                    storage.canonicalize(pair[1]).unwrap().into()
                } else {
                    pair[1]
                };
                let resumed = storage.add_tree(&[
                    Blob::literal(b"").unwrap().into(),
                    continuation,
                    Blob::literal(&0u64.to_le_bytes()).unwrap().into(),
                ]);
                let tag = Tree::try_from(apply(evaluator.runtime(), resumed)).unwrap();
                assert!(tag.is_tag());
                let fields = storage.get_tree(tag).unwrap();
                let expected = match mode {
                    3 => tag_capture.into(),
                    4 => program.into(),
                    5 => storage.get_tree(program).unwrap()[1],
                    _ => unreachable!(),
                };
                assert_eq!(
                    storage.canonicalize(fields[0]).unwrap(),
                    storage.canonicalize(expected).unwrap()
                );
            }
        }

        let portable: Handle = storage
            .canonicalize_tree(Tree::try_from(continuation).unwrap())
            .unwrap()
            .into();
        let canonical_capture =
            Tree::try_from(Handle::from(storage.canonicalize_tree(capture).unwrap())).unwrap();
        let second_request = Tree::try_from(apply(evaluator.runtime(), canonical_capture)).unwrap();
        let second = storage.get_tree(second_request).unwrap()[1];
        let second: Handle = storage
            .canonicalize_tree(Tree::try_from(second).unwrap())
            .unwrap()
            .into();
        assert!(
            portable.same_encoding(&second),
            "checkpoint exposes handle representation"
        );
        let original = storage.get_tree_backing(capture).unwrap();
        let mut mapping = kernel::prelude::Table::new(1 << 30);
        assert!(
            mapping
                .map(
                    0,
                    Entry::ROTable(original.mapped_value().try_into().unwrap())
                )
                .is_ok()
        );
        let backing = storage::ImmutableBytes::from_tables(vec![mapping], original.len());
        let alternative = unsafe { storage.add_tree_backing(backing) };
        let request = Tree::try_from(apply(evaluator.runtime(), alternative)).unwrap();
        let continuation = Tree::try_from(storage.get_tree(request).unwrap()[1]).unwrap();
        assert_eq!(
            storage.canonicalize_tree(continuation).unwrap(),
            CanonicalHandle::try_from(Tree::try_from(portable).unwrap()).unwrap()
        );

        let application = storage.add_tree(&[
            Blob::literal(b"").unwrap().into(),
            portable,
            Blob::literal(&41u64.to_le_bytes()).unwrap().into(),
        ]);
        let result = apply(evaluator.runtime(), application);
        assert_eq!(
            storage
                .get_blob(Blob::try_from(result).unwrap())
                .unwrap()
                .as_ref(),
            &42u64.to_le_bytes()
        );

        let twice = storage.add_tree(&[
            Blob::literal(b"").unwrap().into(),
            program.into(),
            Blob::literal(&2u64.to_le_bytes()).unwrap().into(),
        ]);
        let request = Tree::try_from(apply(evaluator.runtime(), twice)).unwrap();
        let continuation = storage.get_tree(request).unwrap()[1];
        let canonical: Handle = storage
            .canonicalize_tree(Tree::try_from(continuation).unwrap())
            .unwrap()
            .into();
        let mut checkpoints = Vec::new();
        for continuation in [continuation, canonical] {
            let application = storage.add_tree(&[
                Blob::literal(b"").unwrap().into(),
                continuation,
                Blob::literal(&7u64.to_le_bytes()).unwrap().into(),
            ]);
            let request = Tree::try_from(apply(evaluator.runtime(), application)).unwrap();
            let continuation = Tree::try_from(storage.get_tree(request).unwrap()[1]).unwrap();
            checkpoints.push(storage.canonicalize_tree(continuation).unwrap());
        }
        assert_eq!(checkpoints[0], checkpoints[1]);

        let call = storage.add_tree(&[
            Blob::literal(b"").unwrap().into(),
            program.into(),
            Blob::literal(&1u64.to_le_bytes()).unwrap().into(),
        ]);
        let result = evaluator.eval(call.into_ref().application().strict().into());
        assert_eq!(
            storage
                .get_blob(Blob::try_from(result).unwrap())
                .unwrap()
                .as_ref(),
            &42u64.to_le_bytes()
        );
    }

    fn capture_determinism(mode: u64, fixture: &str) -> Vec<(CanonicalHandle, CanonicalHandle)> {
        capture_with_history(mode, fixture, 0)
    }

    fn capture_with_history(
        mode: u64,
        fixture: &str,
        history: usize,
    ) -> Vec<(CanonicalHandle, CanonicalHandle)> {
        let runtime = test_runtime();
        let storage = runtime.storage();
        for index in 0..history {
            let blob = storage.add_blob(&[index as u8; 64]);
            storage.add_tree(&[blob.into()]);
        }
        let program = test_program(storage, kernel::testing::fixture(fixture));
        let metadata: Handle = Blob::literal(b"").unwrap().into();
        let value: Handle = storage.add_blob(&[42; 64]).into();
        let canonical_value: Handle = storage.canonicalize(value).unwrap().into();
        let request = storage.add_tree(&[Blob::literal(b"capture").unwrap().into()]);
        let fields = [
            metadata,
            program.into(),
            value,
            request.into(),
            storage.add_blob(&mode.to_le_bytes()).into(),
        ];
        let original = storage.add_tree(&fields);
        let canonical =
            Tree::try_from(Handle::from(storage.canonicalize(original.into()).unwrap())).unwrap();
        let mut mixed = fields;
        mixed[2] = canonical_value;
        let mixed = storage.add_tree(&mixed);
        let backing = storage.get_tree_backing(original).unwrap();
        let mut mapping = kernel::prelude::Table::new(1 << 30);
        assert!(
            mapping
                .map(
                    0,
                    Entry::ROTable(backing.mapped_value().try_into().unwrap())
                )
                .is_ok()
        );
        let alternate = unsafe {
            storage.add_tree_backing(storage::ImmutableBytes::from_tables(
                vec![mapping],
                backing.len(),
            ))
        };
        let expected = if mode == 2 {
            let mut children = vec![value; 260];
            children[128] = metadata;
            children[129] = metadata;
            storage.add_tree(&children).into()
        } else {
            value
        };
        let expected = storage.canonicalize(expected).unwrap();
        let resume = |continuation| {
            apply(
                &runtime,
                storage.add_tree(&[
                    metadata,
                    continuation,
                    Blob::literal(b"reply").unwrap().into(),
                ]),
            )
        };
        let pair = |result| {
            let fields = storage.get_tree(Tree::try_from(result).unwrap()).unwrap();
            assert_eq!(fields.len(), 2);
            assert_eq!(
                storage.canonicalize(fields[0]).unwrap(),
                storage.canonicalize(request.into()).unwrap()
            );
            fields[1]
        };
        let mut checkpoints = Vec::new();
        for combination in [original, canonical, mixed, alternate] {
            let first = pair(apply(&runtime, combination));
            let second = pair(resume(first));
            assert_eq!(storage.canonicalize(resume(second)).unwrap(), expected);
            let first = storage.canonicalize(first).unwrap();
            let second = storage.canonicalize(second).unwrap();
            let replayed = pair(resume(first.into()));
            assert_eq!(storage.canonicalize(replayed).unwrap(), second);
            for _ in 0..2 {
                assert_eq!(
                    storage.canonicalize(resume(second.into())).unwrap(),
                    expected
                );
            }
            checkpoints.push((first, second));
        }
        for checkpoint in &checkpoints[1..] {
            assert_eq!(
                *checkpoint, checkpoints[0],
                "checkpoint exposes input representation in scenario {mode}"
            );
        }
        checkpoints
    }

    #[kernel::test]
    fn determinism_across_capture_in_exception_handlers() {
        capture_determinism(0, "FIX_CAPTURE_DETERMINISM");
    }

    #[kernel::test]
    fn determinism_across_capture_before_exception_unwind() {
        capture_determinism(1, "FIX_CAPTURE_DETERMINISM");
    }

    #[kernel::test]
    fn determinism_across_bulk_tables_and_replay() {
        capture_determinism(2, "FIX_CAPTURE_DETERMINISM");
    }

    #[kernel::test]
    fn determinism_across_large_table_snapshots() {
        capture_determinism(0, "FIX_LARGE_TABLES");
    }

    #[kernel::test]
    fn determinism_across_exception_references() {
        capture_determinism(3, "FIX_CAPTURE_DETERMINISM");
    }

    #[kernel::test]
    fn determinism_across_handle_operations_and_globals() {
        capture_determinism(0, "FIX_CONTINUATION_STATE");
    }

    #[kernel::test]
    fn determinism_across_attachment_lifetimes_and_funcrefs() {
        capture_determinism(1, "FIX_CONTINUATION_STATE");
    }

    #[kernel::test]
    fn determinism_across_return_cc() {
        capture_determinism(4, "FIX_CONTINUATION_STATE");
    }

    #[kernel::test]
    fn determinism_across_externref_invalidation() {
        capture_determinism(0, "FIX_INVALIDATE_EXTERNREFS");
    }

    #[kernel::test]
    fn determinism_across_partial_tree_attachments() {
        capture_determinism(0, "FIX_PARTIAL_TREE");
    }

    #[kernel::test]
    fn resumption_preserves_results_after_externref_use() {
        let runtime = test_runtime();
        let storage = runtime.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_EXTERNREF_REUSE"));
        for count in [32u64, 512] {
            let combination = storage.add_tree(&[
                Blob::literal(b"").unwrap().into(),
                program.into(),
                Blob::literal(&count.to_le_bytes()).unwrap().into(),
            ]);
            let pair = storage
                .get_tree(Tree::try_from(apply(&runtime, combination)).unwrap())
                .unwrap();
            let resumed = storage.add_tree(&[
                Blob::literal(b"").unwrap().into(),
                pair[1],
                Blob::literal(b"resumed").unwrap().into(),
            ]);
            assert!(
                apply(&runtime, resumed).same_encoding(&Blob::literal(b"resumed").unwrap().into())
            );
        }
    }

    #[kernel::test]
    fn determinism_across_sdk_capture_lifetimes() {
        let runtime = test_runtime();
        let storage = runtime.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_SDK_REUSE"));
        let metadata: Handle = Blob::literal(b"").unwrap().into();
        let pair = |result| {
            let pair = storage.get_tree(Tree::try_from(result).unwrap()).unwrap();
            assert_eq!(pair.len(), 2);
            assert!(pair[0].same_encoding(&Blob::literal(&0u64.to_le_bytes()).unwrap().into()));
            pair[1]
        };
        for count in [32u64, 512] {
            let combination = storage.add_tree(&[
                metadata,
                program.into(),
                Blob::literal(&count.to_le_bytes()).unwrap().into(),
            ]);
            let canonical = Tree::try_from(Handle::from(
                storage.canonicalize(combination.into()).unwrap(),
            ))
            .unwrap();
            let mut checkpoints = Vec::new();
            for combination in [combination, canonical] {
                let first = pair(apply(&runtime, combination));
                let first = storage.canonicalize(first).unwrap();
                let resumed = storage.add_tree(&[metadata, first.into(), metadata]);
                let second = pair(apply(&runtime, resumed));
                let second = storage.canonicalize(second).unwrap();
                let resumed = storage.add_tree(&[metadata, second.into(), metadata]);
                let result = apply(&runtime, resumed);
                assert_eq!(
                    storage
                        .get_blob(Blob::try_from(result).unwrap())
                        .unwrap()
                        .as_ref(),
                    &42u64.to_le_bytes()
                );
                checkpoints.push((first, second));
            }
            assert_eq!(checkpoints[0], checkpoints[1]);
        }
    }

    #[kernel::test]
    fn determinism_across_storage_histories() {
        let expected = capture_with_history(0, "FIX_CONTINUATION_STATE", 0);
        for history in [1, 17] {
            assert_eq!(
                capture_with_history(0, "FIX_CONTINUATION_STATE", history),
                expected
            );
        }
    }

    #[kernel::test]
    fn determinism_across_reply_representations() {
        use fix::storage::memory::MemoryStorage;
        let memory = Arc::new(MemoryStorage::default());
        let author = Blob::canonical(stdlib::PRIMITIVES[0].1)
            .unwrap()
            .object()
            .into_ref();
        let runtime = FixOnArca::new(memory.clone()).with_trust(
            fix::arca::trust::Trust::parse(&format!("{}\n", author.as_handle())).unwrap(),
        );
        let storage = runtime.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_CONTINUATION_STATE"));
        let metadata: Handle = Blob::literal(b"").unwrap().into();
        let value: Handle = storage.add_blob(&[42; 64]).into();
        let canonical: Handle = storage.canonicalize(value).unwrap().into();
        let duplicate: Handle = storage.add_blob(&[42; 64]).into();
        let mut page = Page::new(4096);
        page.write(0, &[42; 64]);
        let mut mapping = kernel::prelude::Table::new(1 << 30);
        assert!(mapping.map(0, Entry::ROPage(page)).is_ok());
        let alternate: Handle = storage
            .add_blob_backing(storage::ImmutableBytes::from_tables(vec![mapping], 64))
            .into();
        let request = storage.add_tree(&[Blob::literal(b"capture").unwrap().into()]);
        let combination = storage.add_tree(&[
            metadata,
            program.into(),
            value,
            request.into(),
            Blob::literal(&3u64.to_le_bytes()).unwrap().into(),
        ]);
        let pair = |result| {
            let fields = storage.get_tree(Tree::try_from(result).unwrap()).unwrap();
            assert_eq!(fields.len(), 2);
            assert_eq!(storage.equals(fields[0], request.into()), Ok(true));
            fields[1]
        };
        let mut checkpoints = Vec::new();
        for reply in [value, canonical, duplicate, alternate] {
            for serialized in [false, true] {
                let first = pair(apply(&runtime, combination));
                let first = if serialized {
                    storage.canonicalize(first).unwrap().into()
                } else {
                    first
                };
                let second = pair(apply(&runtime, storage.add_tree(&[metadata, first, reply])));
                let checkpoint = storage.canonicalize(second).unwrap();
                let resumed = memory.add_tree(&[metadata, checkpoint.into(), value]);
                let fresh = FixOnArca::new(memory.clone());
                let result = fresh
                    .execute_with_effects(resumed, false, &mut |_| panic!("unexpected effect"))
                    .unwrap();
                assert_eq!(storage.equals(result, value), Ok(true));
                checkpoints.push(checkpoint);
            }
        }
        assert!(
            checkpoints
                .iter()
                .all(|checkpoint| *checkpoint == checkpoints[0])
        );
    }

    #[kernel::test]
    fn determinism_across_eval_and_call_cc() {
        for (fixture, mode) in [("FIX_EVAL", 0u64), ("FIX_EVAL", 1), ("FIX_CONTINUATION", 1)] {
            let evaluator = Evaluation::new(test_runtime());
            let storage = evaluator.storage();
            let program = test_program(storage, kernel::testing::fixture(fixture));
            let metadata: Handle = Blob::literal(b"").unwrap().into();
            let argument = Blob::literal(&41u64.to_le_bytes()).unwrap();
            let mode = Blob::literal(&mode.to_le_bytes()).unwrap();
            let fields = if fixture == "FIX_EVAL" {
                vec![metadata, program.into(), argument.into(), mode.into()]
            } else {
                vec![metadata, program.into(), mode.into()]
            };
            let original = storage.add_tree(&fields);
            let canonical =
                Tree::try_from(Handle::from(storage.canonicalize(original.into()).unwrap()))
                    .unwrap();
            let mut checkpoints = Vec::new();
            for combination in [original, canonical] {
                let result = apply(evaluator.runtime(), combination);
                checkpoints.push(storage.canonicalize(result).unwrap());
                let result = evaluator.eval(Thunk::try_from(result).unwrap().strict().into());
                assert_eq!(
                    storage
                        .get_blob(Blob::try_from(result).unwrap())
                        .unwrap()
                        .as_ref(),
                    &42u64.to_le_bytes()
                );
            }
            assert_eq!(checkpoints[0], checkpoints[1]);
        }
    }

    #[kernel::test]
    fn determinism_during_concurrent_replay_and_materialization() {
        use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

        let runtime = Arc::new(test_runtime());
        let storage = runtime.storage();
        let program = test_program(storage, kernel::testing::fixture("FIX_CONTINUATION"));
        let metadata: Handle = Blob::literal(b"").unwrap().into();
        let combination = storage.add_tree(&[
            metadata,
            program.into(),
            Blob::literal(&0u64.to_le_bytes()).unwrap().into(),
        ]);
        let capture = || {
            let pair = Tree::try_from(apply(&*runtime, combination)).unwrap();
            storage.get_tree(pair).unwrap()[1]
        };
        let continuation = capture();
        let expected = storage.canonicalize(capture()).unwrap();
        let ready = Arc::new(AtomicUsize::new(0));
        let start = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicUsize::new(0));
        for index in 0..4 {
            let runtime = runtime.clone();
            let ready = ready.clone();
            let start = start.clone();
            let finished = finished.clone();
            kernel::kthread::spawn(move || {
                ready.fetch_add(1, Ordering::Release);
                while !start.load(Ordering::Acquire) {
                    kernel::kthread::yield_now();
                }
                let storage = runtime.storage();
                if index < 2 {
                    let reply = 7u64 + index;
                    let combination = storage.add_tree(&[
                        metadata,
                        continuation,
                        Blob::literal(&reply.to_le_bytes()).unwrap().into(),
                    ]);
                    let result = apply(&*runtime, combination);
                    assert_eq!(
                        storage.canonicalize(result).unwrap(),
                        storage
                            .canonicalize(Blob::literal(&(reply + 1).to_le_bytes()).unwrap().into())
                            .unwrap()
                    );
                } else {
                    assert_eq!(storage.canonicalize(continuation).unwrap(), expected);
                }
                finished.fetch_add(1, Ordering::Release);
            });
        }
        while ready.load(Ordering::Acquire) != 4 {
            kernel::kthread::yield_now();
        }
        start.store(true, Ordering::Release);
        while finished.load(Ordering::Acquire) != 4 {
            kernel::kthread::yield_now();
        }
        assert_eq!(storage.canonicalize(continuation).unwrap(), expected);
    }

    #[kernel::test]
    fn canonical_blob_round_trip() {
        let storage = storage::memory::MemoryStorage::default();
        let bytes = [42u8; 64];
        let local = storage.add_blob(&bytes);
        let canonical = storage.canonicalize_blob(local).unwrap();
        let handle = Handle::from(canonical);
        let blob = Blob::try_from(handle).unwrap();
        assert_eq!(storage.get_blob(blob).unwrap().as_ref(), &bytes);

        let duplicate = storage.add_blob(&bytes);
        assert_eq!(storage.canonicalize_blob(duplicate), Ok(canonical));

        let decoded =
            Blob::try_from(unsafe { Handle::parse(*handle.as_bytes()) }.unwrap()).unwrap();
        assert_eq!(CanonicalHandle::try_from(decoded).unwrap(), canonical);
        assert_eq!(storage.get_blob(decoded).unwrap().as_ref(), &bytes);
    }

    #[kernel::test]
    fn canonical_tree_round_trip() {
        fn build(storage: &storage::memory::MemoryStorage) -> Tree {
            let blob = storage.add_blob(&[42; 64]);
            let tree = storage.add_tree(&[blob.into(), Blob::literal(b"leaf").unwrap().into()]);
            storage.add_tree(&[
                blob.into(),
                tree.into(),
                tree.tag_descriptor().into(),
                blob.object().into_ref().into(),
                tree.into_ref().into(),
                blob.object().into_ref().identification().into(),
                tree.into_ref().application().into(),
                tree.into_ref().selection().into(),
                tree.into_ref().application().strict().into(),
                tree.into_ref().selection().shallow().into(),
            ])
        }

        let storage = storage::memory::MemoryStorage::default();
        let other = storage::memory::MemoryStorage::default();
        let original = build(&storage);
        let original_children = storage.get_tree(original).unwrap();
        let canonical = storage.canonicalize_tree(original).unwrap();
        assert_eq!(
            storage.canonicalize_tree(build(&storage)).unwrap(),
            canonical
        );
        assert_eq!(other.canonicalize_tree(build(&other)).unwrap(), canonical);
        assert_eq!(
            storage
                .canonicalize_tree(Tree::try_from(Handle::from(canonical)).unwrap())
                .unwrap(),
            canonical
        );

        let children = storage
            .get_tree(Tree::try_from(Handle::from(canonical)).unwrap())
            .unwrap();
        for (original, child) in original_children.iter().zip(children.iter()) {
            assert_eq!(original.as_bytes()[31], child.as_bytes()[31]);
            assert!(child.is_canonical());
        }
        let blob = Blob::try_from(children[0]).unwrap();
        assert_eq!(storage.get_blob(blob).unwrap().as_ref(), &[42; 64]);
        let tree = Tree::try_from(children[1]).unwrap();
        let tagged = Tree::try_from(children[2]).unwrap();
        assert!(tagged.is_tag());
        let nested = storage.get_tree(tree).unwrap();
        let tagged_children = storage.get_tree(tagged).unwrap();
        assert_eq!(nested[0].as_bytes(), tagged_children[0].as_bytes());
        assert_eq!(
            Blob::try_from(nested[1]).unwrap().literal_bytes().unwrap(),
            b"leaf"
        );
        assert_eq!(
            storage
                .get_blob(Blob::try_from(nested[0]).unwrap())
                .unwrap()
                .as_ref(),
            &[42; 64]
        );
    }

    #[kernel::test]
    fn equality_of_aliases_and_computed_values() {
        use fix::storage::{StorageError, memory::MemoryStorage};
        let storage = MemoryStorage::default();
        let a = storage.add_blob(&[7; 64]);
        let b = storage.add_blob(&[7; 64]);
        let c = storage.add_blob(&[8; 64]);
        assert!(a.as_handle().is_comparable());
        assert_eq!(storage.equals(a.into(), b.into()), Ok(true));
        assert_eq!(storage.equals(a.into(), c.into()), Ok(false));
        let canonical: Handle = storage.canonicalize_blob(a).unwrap().into();
        assert_eq!(storage.equals(a.into(), canonical), Ok(true));
        assert_eq!(storage.equals(a.into(), a.into_ref().into()), Ok(false));
        assert_eq!(
            storage.equals(a.into_ref().into(), b.into_ref().into()),
            Ok(true)
        );
        let left = storage.add_tree(&[a.into(), storage.add_tree(&[a.into_ref().into()]).into()]);
        let right = storage.add_tree(&[b.into(), storage.add_tree(&[b.into_ref().into()]).into()]);
        assert!(left.as_handle().is_comparable());
        assert_eq!(storage.equals(left.into(), right.into()), Ok(true));
        let canonical: Handle = storage.canonicalize_tree(left).unwrap().into();
        assert_eq!(storage.equals(right.into(), canonical), Ok(true));
        let other = MemoryStorage::default();
        assert_eq!(other.equals(canonical, canonical), Ok(true));
        let reference = Object::try_from(canonical).unwrap().into_ref().handle();
        assert_eq!(other.equals(reference, reference), Ok(true));
        assert_eq!(
            storage.equals(left.into_ref().into(), right.into_ref().into()),
            Ok(true)
        );
        let different = storage.add_tree(&[
            b.into_ref().into(),
            storage.add_tree(&[b.into_ref().into()]).into(),
        ]);
        assert_eq!(storage.equals(left.into(), different.into()), Ok(false));
        let thunk = a.into_ref().identification();
        for value in [
            Handle::from(thunk),
            thunk.strict().into(),
            thunk.shallow().into(),
            left.into_ref().application().into(),
        ] {
            assert!(!value.is_comparable());
            assert_eq!(
                storage.equals(value, value),
                Err(StorageError::NotComparable)
            );
            let tree = storage.add_tree(&[value]);
            let canonical: Handle = storage.canonicalize_tree(tree).unwrap().into();
            assert!(canonical.is_canonical());
            assert!(!canonical.is_comparable());
            assert_eq!(
                storage.equals(canonical, canonical),
                Err(StorageError::NotComparable)
            );
            assert!(
                !Tree::try_from(canonical)
                    .unwrap()
                    .into_ref()
                    .as_handle()
                    .is_comparable()
            );
        }
        let empty = storage.add_tree(&[]);
        assert!(empty.as_handle().is_comparable());
        assert_eq!(storage.equals(empty.into(), empty.into()), Ok(true));
        for (bit, error) in [
            (4, StorageError::InvalidComparisonBit),
            (8, StorageError::InvalidValueBit),
        ] {
            let mut forged = *left.as_handle().as_bytes();
            forged[30] ^= bit;
            assert_eq!(
                storage
                    .get_tree(Tree::try_from(unsafe { Handle::parse(forged) }.unwrap()).unwrap())
                    .unwrap_err(),
                error
            );
        }
    }

    #[kernel::test]
    fn equality_after_encoded_substitution() {
        let evaluator = Evaluation::new(test_runtime());
        let storage = evaluator.storage();
        let blob = storage.add_blob(&[42; 64]);
        let identification = blob.into_ref().identification().strict();
        let source = storage.add_tree(&[blob.into()]);
        let index = storage.add_blob(&0u64.to_le_bytes());
        let selection = storage
            .add_tree(&[source.into_ref().into(), index.into()])
            .into_ref()
            .selection()
            .strict();
        let first = storage.add_tree(&[identification.into()]);
        let second = storage.add_tree(&[selection.into()]);
        assert!(!first.as_handle().is_comparable() && !second.as_handle().is_comparable());
        assert_ne!(
            storage.canonicalize_tree(first).unwrap(),
            storage.canonicalize_tree(second).unwrap()
        );
        let first = evaluator.eval(
            first
                .into_ref()
                .reference()
                .identification()
                .strict()
                .into(),
        );
        let second = evaluator.eval(
            second
                .into_ref()
                .reference()
                .identification()
                .strict()
                .into(),
        );
        assert!(first.is_comparable() && second.is_comparable());
        assert_eq!(storage.equals(first, second), Ok(true));
    }

    #[kernel::test]
    fn userspace_evaluation_preserves_access_and_tags() {
        fn check(storage: &dyn storage::Storage, eval: impl Fn(Handle) -> Handle) {
            let blob = storage.add_blob(b"ok");
            let thunk = blob.into_ref().identification();
            assert!(matches!(
                eval(thunk.shallow().into()).view(),
                HandleView::Ref(_)
            ));
            assert!(matches!(
                eval(thunk.strict().into()).view(),
                HandleView::Object(_)
            ));

            let index = storage.add_blob(&0u64.to_le_bytes());
            let invalid_index = storage.add_blob(&99u64.to_le_bytes());
            let invalid = storage.add_tree(&[blob.into_ref().into(), invalid_index.into()]);
            let source =
                storage.add_tree(&[blob.into(), invalid.into_ref().selection().strict().into()]);
            for index in [
                Handle::from(index),
                index.into_ref().identification().strict().into(),
            ] {
                let selection = storage.add_tree(&[source.into_ref().into(), index]);
                let selected = eval(selection.into_ref().selection().strict().into());
                assert_eq!(storage.equals(selected, blob.into()), Ok(true));
            }

            let tag = storage
                .add_tree(&[
                    storage.add_tree(&[]).into(),
                    thunk.strict().into(),
                    blob.into(),
                ])
                .tag_descriptor();
            let result = Tree::try_from(eval(tag.into())).unwrap();
            assert!(result.is_tag());
            assert!(
                storage
                    .get_tree(result)
                    .unwrap()
                    .iter()
                    .all(Handle::is_comparable)
            );
            let plain = storage.add_tree(&storage.get_tree(result).unwrap());
            assert_eq!(storage.equals(result.into(), plain.into()), Ok(false));

            let leaf = storage.add_tree(&[blob.into()]);
            let pending = storage.add_tree(&[thunk.strict().into()]).into_ref();
            assert_eq!(
                storage.equals(pending.into(), pending.into()),
                Err(storage::StorageError::NotComparable)
            );
            assert_eq!(
                storage.equals(leaf.into_ref().into(), leaf.into_ref().into()),
                Ok(true)
            );
            assert_eq!(storage.equals(eval(leaf.into()), leaf.into()), Ok(true));
            for target in [Handle::from(blob), Handle::from(leaf)] {
                let end = storage.add_blob(&(target.len() as u64).to_le_bytes());
                let entire = storage.add_tree(&[target, index.into(), end.into()]);
                let entire = eval(entire.into_ref().selection().strict().into());
                assert_eq!(storage.equals(entire, target), Ok(true));
                let empty = storage.add_tree(&[target, end.into(), end.into()]);
                let empty = eval(empty.into_ref().selection().strict().into());
                assert_eq!(empty.len(), 0);
            }
            let references = storage.add_tree(&[leaf.into_ref().into(), blob.into_ref().into()]);
            let loaded = eval(references.into());
            let children = storage.get_tree(Tree::try_from(loaded).unwrap()).unwrap();
            assert!(
                children
                    .iter()
                    .all(|child| matches!(child.view(), HandleView::Ref(_)))
            );
            assert_eq!(eval(references.into()).as_bytes(), loaded.as_bytes());
        }
        for workers in [1, 2] {
            let evaluator = Evaluation::with_workers(test_runtime(), workers);
            check(evaluator.storage(), |value| {
                let expected = fix_reference::eval(evaluator.runtime(), value);
                let actual = evaluator.eval(value);
                let expected: Handle = evaluator.storage().canonicalize(expected).unwrap().into();
                let actual_name: Handle = evaluator.storage().canonicalize(actual).unwrap().into();
                assert!(actual_name.same_encoding(&expected));
                actual
            });
        }
    }

    fn eval_value(source: &str, evaluator: &Evaluation) -> Vec<u8> {
        let handle = eval_program(source, evaluator);
        let object = match handle.view() {
            HandleView::Object(object) => *object,
            HandleView::Ref(reference) => reference.object_descriptor(),
            _ => panic!(),
        };
        match object.view() {
            ObjectView::Blob(blob) => evaluator.storage().get_blob(*blob).unwrap().into(),
            ObjectView::Tree(tree) => tree.len().to_le_bytes().into(),
        }
    }

    #[kernel::test]
    fn test_number() {
        let evaluator = Evaluation::new(test_runtime());

        {
            assert_eq!(eval_value("42u8", &evaluator), 42u8.to_le_bytes());
            //assert_eq!(eval_value("-1", &evaluator), (-1i64).to_le_bytes());
            assert_eq!(eval_value("\"hello\"", &evaluator), b"hello");

            assert_eq!(eval_value("(1u8 2u8 3u8)", &evaluator), 3u64.to_le_bytes());
            assert_eq!(eval_value("()", &evaluator), 0u64.to_le_bytes());

            assert_eq!(eval_value("&\"hello\"", &evaluator), b"hello");
            assert_eq!(eval_value("&(1u8 2u8 3u8)", &evaluator), 3u64.to_le_bytes());

            assert_eq!(eval_value("*'&2u8", &evaluator), 2u8.to_le_bytes());
            assert_eq!(
                eval_value("*[(1u8 2u8 3u8 4u8) 2u64]", &evaluator),
                3u8.to_le_bytes()
            );

            assert_eq!(eval_value("x = 42u64\nx", &evaluator), 42u64.to_le_bytes());
            assert_eq!(
                eval_value("x = 1u8\ny = 2u8\n(x y)", &evaluator),
                2u64.to_le_bytes()
            );

            assert_eq!(
                eval_value("x = 1u8\nx = 2u64\nx", &evaluator),
                2u64.to_le_bytes()
            );
            assert_eq!(
                eval_value("x = 1u64\ny = x\nx = 2u8\ny", &evaluator),
                1u64.to_le_bytes()
            );
        }

        // primitive test
        {
            assert_eq!(
                eval_value("*#(\"\" $identity 2u8)", &evaluator),
                3u64.to_le_bytes()
            );
        }
    }
}
