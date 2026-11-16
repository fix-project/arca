use super::{evaluation::Evaluation, test_program, test_runtime};
use fixruntime::{Blob, Handle, Runtime, Thunk, Tree, stdlib};
use kernel::prelude::Vec;

#[kernel::test]
fn scheduler_tcp_listener_round_trip() {
    use fixruntime::user_scheduler_driver::{PersistenceOnly, start};
    use kernel::prelude::Arc;

    let runtime = Arc::new(test_runtime());
    let storage = runtime.storage();
    let program = test_program(storage, kernel::testing::fixture("FIX_TCP_LISTENER"));
    let combination = storage.add_tree(&[Tree::empty().into(), program.into()]);
    let report = start(runtime.clone(), combination, 1, PersistenceOnly(Ok)).unwrap();
    let result = Blob::try_from(report.result.unwrap()).unwrap();
    assert_eq!(storage.get_blob(result).unwrap().as_ref(), b"roundtrip");
}

#[kernel::test]
fn scheduler_evaluates_wide_application_graphs() {
    for evaluator in [
        Evaluation::new(test_runtime()),
        Evaluation::with_workers(test_runtime(), 2),
        Evaluation::with_scheduler(test_runtime(), 2, "FIX_ONEPASS_SCHEDULER"),
        Evaluation::with_scheduler(test_runtime(), 2, "FIX_HINT_SCHEDULER"),
    ] {
        let storage = evaluator.storage();
        let program = test_program(storage, stdlib::PRIMITIVES[0].1);
        let values: Vec<Handle> = (0u64..256)
            .map(|index| {
                let value = storage.add_blob(&index.to_le_bytes());
                let leaf = storage.add_tree(&[value.into()]);
                let data = storage.add_tree(&[leaf.into()]);
                storage
                    .add_tree(&[
                        Blob::literal(b"").unwrap().into(),
                        program.into(),
                        data.into(),
                    ])
                    .into()
            })
            .collect();
        let encodes: Vec<_> = values
            .iter()
            .map(|value| {
                Tree::try_from(*value)
                    .unwrap()
                    .into_ref()
                    .application()
                    .strict()
                    .into()
            })
            .collect();
        let input = storage.add_tree(&encodes);
        let expected = storage.add_tree(&values);
        let result = evaluator.eval(input.into());
        assert!(storage.equals(result, expected.into()).unwrap());
    }
}

#[kernel::test]
fn scheduler_resolves_long_force_chains() {
    for evaluator in [
        Evaluation::new(test_runtime()),
        Evaluation::with_workers(test_runtime(), 2),
    ] {
        let storage = evaluator.storage();
        let expected: Handle = storage.add_blob(b"result").into();
        let mut input = expected;
        for _ in 0..256 {
            let values = storage.add_tree(&[input]);
            let arguments =
                storage.add_tree(&[values.into(), storage.add_blob(&0u64.to_le_bytes()).into()]);
            input = arguments.into_ref().selection().into();
        }
        let input = Thunk::try_from(input).unwrap().strict();
        assert!(
            storage
                .equals(evaluator.eval(input.into()), expected)
                .unwrap()
        );
    }
}

#[kernel::test]
fn scheduler_thinks_once_and_forces_through_thunks() {
    let evaluator = Evaluation::new(test_runtime());
    let storage = evaluator.storage();
    let expected: Handle = storage.add_blob(b"result").into();
    let mut input = expected;
    let mut first_step = expected;
    for _ in 0..4 {
        first_step = input;
        let values = storage.add_tree(&[input]);
        let arguments = storage.add_tree(&[
            values.into(),
            Blob::literal(&0u64.to_le_bytes()).unwrap().into(),
        ]);
        input = storage
            .canonicalize(arguments.into_ref().selection().into())
            .unwrap()
            .into();
    }
    let thunk = Thunk::try_from(input).unwrap();
    assert!(evaluator.think(thunk).same_encoding(&first_step));
    assert!(
        storage
            .equals(evaluator.eval(thunk.strict().into()), expected)
            .unwrap()
    );
}

#[kernel::test]
fn scheduler_evaluates_shared_dependencies() {
    for evaluator in [
        Evaluation::new(test_runtime()),
        Evaluation::with_workers(test_runtime(), 1),
        Evaluation::with_workers(test_runtime(), 4),
        Evaluation::with_scheduler(test_runtime(), 4, "FIX_RECOMPUTE_SCHEDULER"),
    ] {
        let storage = evaluator.storage();
        let program = test_program(storage, stdlib::PRIMITIVES[0].1);
        let shared = storage.add_tree(&[Blob::literal(b"shared").unwrap().into(), program.into()]);
        let encoded = shared.into_ref().application().strict();
        let mut inputs = Vec::new();
        let mut expected = Vec::new();
        for index in 0u64..64 {
            let metadata = storage.add_blob(&index.to_le_bytes());
            let input = storage.add_tree(&[
                metadata.into(),
                program.into(),
                encoded.into(),
                encoded.into(),
            ]);
            inputs.push(input.into_ref().application().strict().into());
            expected.push(
                storage
                    .add_tree(&[
                        metadata.into(),
                        program.into(),
                        shared.into(),
                        shared.into(),
                    ])
                    .into(),
            );
        }
        let input = storage.add_tree(&inputs);
        let expected = storage.add_tree(&expected);
        let result = evaluator.eval(input.into());
        assert!(storage.equals(result, expected.into()).unwrap());
        assert!(
            storage
                .equals(evaluator.eval(input.into()), result)
                .unwrap()
        );
    }
}

#[kernel::test]
fn scheduler_preserves_tree_order_and_duplicate_premises() {
    for evaluator in [
        Evaluation::new(test_runtime()),
        Evaluation::with_workers(test_runtime(), 1),
        Evaluation::with_workers(test_runtime(), 4),
        Evaluation::with_scheduler(test_runtime(), 4, "FIX_RECOMPUTE_SCHEDULER"),
    ] {
        let storage = evaluator.storage();
        let program = test_program(storage, stdlib::PRIMITIVES[0].1);
        let values: Vec<Handle> = (0u64..16)
            .map(|index| {
                storage
                    .add_tree(&[
                        storage.add_blob(&index.to_le_bytes()).into(),
                        program.into(),
                    ])
                    .into()
            })
            .collect();
        let order = [7, 1, 7, 15, 0, 1, 8, 15, 3, 8, 7, 0];
        let inputs: Vec<Handle> = order
            .iter()
            .map(|&index| {
                Tree::try_from(values[index])
                    .unwrap()
                    .into_ref()
                    .application()
                    .strict()
                    .into()
            })
            .collect();
        let expected: Vec<Handle> = order.iter().map(|&index| values[index]).collect();
        let input = storage.add_tree(&inputs);
        let expected = storage.add_tree(&expected);
        let result = evaluator.eval(input.into());
        assert!(storage.equals(result, expected.into()).unwrap());
        assert!(
            storage
                .equals(evaluator.eval(input.into()), result)
                .unwrap()
        );
    }
}

#[kernel::test]
fn scheduler_resumes_requests_blocked_on_shared_dependencies() {
    for evaluator in [
        Evaluation::new(test_runtime()),
        Evaluation::with_workers(test_runtime(), 1),
        Evaluation::with_workers(test_runtime(), 4),
        Evaluation::with_scheduler(test_runtime(), 4, "FIX_RECOMPUTE_SCHEDULER"),
    ] {
        let storage = evaluator.storage();
        let program = test_program(storage, stdlib::PRIMITIVES[0].1);
        let leaf: Handle = storage.add_blob(b"leaf").into();
        let mut inputs = [leaf, leaf];
        let mut values = [leaf, leaf];
        for index in 0u64..32 {
            let metadata: Handle = storage.add_blob(&index.to_le_bytes()).into();
            let combination = storage.add_tree(&[metadata, program.into(), inputs[1], inputs[0]]);
            let input = combination.into_ref().application().strict().into();
            let value = storage
                .add_tree(&[metadata, program.into(), values[1], values[0]])
                .into();
            inputs = [inputs[1], input];
            values = [values[1], value];
        }
        let result = evaluator.eval(inputs[1]);
        assert!(storage.equals(result, values[1]).unwrap());
    }
}

#[kernel::test]
fn collector_rejects_witness_lists_above_its_limit() {
    let runtime = test_runtime();
    let storage = runtime.storage();
    let collector = test_program(storage, kernel::testing::fixture("FIX_COLLECTOR"));
    let value: Handle = storage.add_blob(b"leaf").into();
    let equivalent = storage.add_blob(&0u64.to_le_bytes());
    let claim = storage.add_tree(&[equivalent.into(), value, value]);
    let kernel_author = storage.add_tree(&[]);
    let tag = storage
        .add_tree(&[kernel_author.into(), claim.into_ref().into()])
        .tag_descriptor();
    let witness = storage.add_tree(&[tag.into(), claim.into()]);
    let count = fixruntime::coupon::MAX_WITNESSES + 1;
    let fields = storage.add_tree(&kernel::prelude::vec![value; count]);
    let witnesses = storage.add_tree(&kernel::prelude::vec![witness.into(); count]);
    let request = storage.add_tree(&[
        Blob::literal(b"").unwrap().into(),
        collector.into(),
        equivalent.into(),
        fields.into(),
        fields.into(),
        witnesses.into(),
    ]);
    let result = runtime.execute_privileged(request).unwrap();
    let result = Tree::try_from(result).unwrap();
    assert!(!result.is_tag());
    let fields = storage.get_tree(result).unwrap();
    assert_eq!(fields.len(), 2);
    assert_eq!(
        storage
            .get_blob(Blob::try_from(fields[0]).unwrap())
            .unwrap()
            .as_ref(),
        b"Wasm.trap"
    );
    assert_eq!(
        storage
            .get_blob(Blob::try_from(fields[1]).unwrap())
            .unwrap()
            .as_ref(),
        5u32.to_le_bytes()
    );
}

#[kernel::test]
fn rust_sdk_returns_complete_panic_messages() {
    let runtime = test_runtime();
    let storage = runtime.storage();
    for fixture in ["FIX_SDK_PANIC", "FIX_SDK_STD_PANIC"] {
        let program = test_program(storage, kernel::testing::fixture(fixture));
        let long_message = kernel::prelude::vec![b'x'; 70000];
        for (mode, expected) in [
            (0u64, b"simple panic".as_slice()),
            (1, "failed λ with value 42".as_bytes()),
            (2, long_message.as_slice()),
            (3, b"Fix procedure failed: WrongType".as_slice()),
            (4, b"owned panic message".as_slice()),
        ] {
            if mode == 4 && fixture != "FIX_SDK_STD_PANIC" {
                continue;
            }
            let request = storage.add_tree(&[
                Blob::literal(b"").unwrap().into(),
                program.into(),
                storage.add_blob(&mode.to_le_bytes()).into(),
            ]);
            let result = runtime.execute_privileged(request).unwrap();
            let repeated = runtime.execute_privileged(request).unwrap();
            assert!(storage.equals(result, repeated).unwrap());
            let result = Tree::try_from(result).unwrap();
            assert!(!result.is_tag());
            let fields = storage.get_tree(result).unwrap();
            assert_eq!(fields.len(), 2);
            assert_eq!(
                storage
                    .get_blob(Blob::try_from(fields[0]).unwrap())
                    .unwrap()
                    .as_ref(),
                b"Rust.panic"
            );
            assert_eq!(
                storage
                    .get_blob(Blob::try_from(fields[1]).unwrap())
                    .unwrap()
                    .as_ref(),
                expected
            );
        }
        let request = storage.add_tree(&[
            Blob::literal(b"").unwrap().into(),
            program.into(),
            storage.add_blob(&5u64.to_le_bytes()).into(),
        ]);
        let result = runtime.execute_privileged(request).unwrap();
        assert_eq!(
            storage
                .get_blob(Blob::try_from(result).unwrap())
                .unwrap()
                .as_ref(),
            b"early return"
        );
    }
}

#[kernel::test]
fn scheduler_controls_memoization_without_losing_shared_results() {
    for (fixture, repeated) in [("FIX_SCHEDULER", 0), ("FIX_RECOMPUTE_SCHEDULER", 1)] {
        let evaluator = Evaluation::with_scheduler(test_runtime(), 4, fixture);
        let storage = evaluator.storage();
        let program = test_program(storage, stdlib::PRIMITIVES[0].1);
        let combination = storage.add_tree(&[Blob::literal(b"").unwrap().into(), program.into()]);
        let input = storage.add_tree(
            &kernel::prelude::vec![combination.into_ref().application().strict().into(); 64],
        );
        let expected = storage.add_tree(&kernel::prelude::vec![combination.into(); 64]);
        for count in [1, repeated] {
            let (result, executed) = evaluator.eval_count(input.into());
            assert!(storage.equals(result, expected.into()).unwrap());
            assert_eq!(executed, count);
        }
        let claims = evaluator
            .runtime()
            .cached_claims_in(
                &[
                    Blob::literal(&2u64.to_le_bytes()).unwrap().into(),
                    combination.into(),
                ],
                &fixruntime::arca::SavedClaims::default(),
            )
            .unwrap();
        assert_eq!(claims.is_empty(), fixture == "FIX_RECOMPUTE_SCHEDULER");
    }
}

#[kernel::test]
fn scheduler_placement_policies() {
    let runtime = test_runtime();
    let storage = runtime.storage();
    let program = test_program(storage, kernel::testing::fixture("FIX_POLICY_CHECKS"));
    let advice = test_program(storage, kernel::testing::fixture("FIX_ADVICE"));
    let advised = storage.add_tree(&[Blob::literal(b"").unwrap().into(), advice.into()]);
    let collector = test_program(storage, kernel::testing::fixture("FIX_COLLECTOR"));
    let combination = storage.add_tree(&[
        Blob::literal(b"").unwrap().into(),
        program.into(),
        advised.into(),
        collector.into(),
    ]);
    let compiler = Blob::canonical(stdlib::PRIMITIVES[0].1)
        .unwrap()
        .object()
        .into_ref();
    let mut trust = fixruntime::arca::trust::Trust::parse(&kernel::prelude::format!(
        "{}\n",
        compiler.as_handle()
    ))
    .unwrap();
    trust.add_tagger(storage, collector).unwrap();
    let runtime = kernel::prelude::Arc::new(runtime.with_trust(trust));
    let storage = runtime.storage();
    let result = fixruntime::user_scheduler_driver::start(
        runtime.clone(),
        combination,
        1,
        fixruntime::user_scheduler_driver::PersistenceOnly(Ok),
    )
    .unwrap()
    .result
    .unwrap();
    if let Ok(tree) = Tree::try_from(result) {
        let fields = storage.get_tree(tree).unwrap();
        if let Some(message) = fields.get(1).and_then(|value| Blob::try_from(*value).ok()) {
            panic!(
                "policy checks returned an error: {}",
                kernel::prelude::String::from_utf8_lossy(
                    storage.get_blob(message).unwrap().as_ref()
                )
            );
        }
    }
    assert!(
        storage
            .equals(result, Blob::literal(b"ok").unwrap().into())
            .unwrap()
    );
}
