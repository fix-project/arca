use super::{test_program, test_runtime};
use fixruntime::{Blob, Handle, Runtime, Tree, arca::Execution};
use kernel::prelude::Vec;

#[kernel::test]
fn advice_events_preserve_order_targets_and_application_identity() {
    let runtime = test_runtime();
    let storage = runtime.storage();
    let program = test_program(storage, kernel::testing::fixture("FIX_ADVICE"));
    let combination = storage.add_tree(&[
        Blob::literal(b"").unwrap().into(),
        program.into(),
        storage.add_blob(b"argument").into(),
    ]);
    let unobserved = runtime.execute(combination).unwrap();
    let expected: &[(&[u8], &[u8])] = &[
        (b"fix::title", b"probe"),
        (b"fix::progress", b"first"),
        (b"fix::progress", b"last"),
        (b"sample::binary", &[0, 255]),
        (b"sample::local", b"probe"),
        (b"sample::local", b"probe"),
    ];
    for privileged in [false, true] {
        let mut execution = runtime.execute_observed(combination, privileged).unwrap();
        let mut events = Vec::new();
        let mut local_targets = Vec::new();
        let contents = b"freshly constructed advice target contents";
        let check_local = |target: Handle| {
            let blob = if let Ok(tree) = Tree::try_from(target) {
                let children = storage.get_tree(tree).unwrap();
                assert_eq!(children.len(), 2);
                assert!(storage.equals(children[1], combination.into()).unwrap());
                Blob::try_from(children[0]).unwrap()
            } else {
                Blob::try_from(target).unwrap()
            };
            assert_eq!(storage.get_blob(blob).unwrap().as_ref(), contents);
        };
        let result = loop {
            match execution {
                Execution::Complete(value) => break value,
                Execution::Advice { event, suspended } => {
                    if event.key == b"sample::local" {
                        assert_eq!(
                            Tree::try_from(event.target).is_ok(),
                            local_targets.len() == 1
                        );
                        check_local(event.target);
                        local_targets.push(event.target);
                    } else {
                        assert!(storage.equals(event.target, combination.into()).unwrap());
                    }
                    events.push((event.key, event.payload));
                    execution = runtime.resume_observed(suspended).unwrap();
                }
            }
        };
        assert_eq!(local_targets.len(), 2);
        for target in local_targets {
            check_local(target);
        }
        let actual: Vec<_> = events
            .iter()
            .map(|(key, payload)| (key.as_slice(), payload.as_slice()))
            .collect();
        assert_eq!(actual.as_slice(), expected);
        let expected: Handle = if privileged {
            combination.into()
        } else {
            unobserved.into()
        };
        assert!(storage.equals(result, expected).unwrap());
        let actual = storage.canonicalize(result).unwrap();
        let expected = storage.canonicalize(expected).unwrap();
        assert_eq!(actual, expected);
    }
}

#[kernel::test]
fn suspended_advice_is_owned_by_its_runtime() {
    let runtime = test_runtime();
    let storage = runtime.storage();
    let program = test_program(storage, kernel::testing::fixture("FIX_ADVICE"));
    let combination = storage.add_tree(&[Blob::literal(b"").unwrap().into(), program.into()]);
    let Execution::Advice { suspended, .. } = runtime.execute_observed(combination, false).unwrap()
    else {
        panic!("expected advice")
    };
    assert!(test_runtime().resume_observed(suspended).is_err());
}

#[kernel::test]
fn advice_preserves_canonical_checkpoint_identity() {
    let runtime = test_runtime();
    let storage = runtime.storage();
    let program = test_program(storage, kernel::testing::fixture("FIX_ADVICE_CAPTURE"));
    let combination = storage.add_tree(&[
        Blob::literal(b"").unwrap().into(),
        program.into(),
        storage.add_blob(b"captured argument").into(),
    ]);
    for privileged in [false, true] {
        let unobserved = if privileged {
            runtime.execute_privileged(combination).unwrap()
        } else {
            runtime.execute(combination).unwrap().into()
        };
        let Execution::Advice { event, suspended } =
            runtime.execute_observed(combination, privileged).unwrap()
        else {
            panic!("expected advice before capture")
        };
        assert!(storage.equals(event.target, combination.into()).unwrap());
        assert_eq!(event.key, b"fix::progress");
        assert_eq!(event.payload, b"before capture");
        let Execution::Complete(observed) = runtime.resume_observed(suspended).unwrap() else {
            panic!("expected captured effect")
        };
        assert!(storage.equals(observed, unobserved).unwrap());
        assert_eq!(
            storage.canonicalize(observed).unwrap(),
            storage.canonicalize(unobserved).unwrap()
        );
    }
}
