#![no_main]
#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(kernel::testing::harness)]
#![reexport_test_harness_main = "test_main"]
extern crate alloc;
use alloc::sync::Arc;
use fixruntime::{
    Blob, Handle, Storage, Tree, TreeRef,
    arca::{FixOnArca, trust::Trust},
    runtime::Runtime,
    stdlib,
    storage::memory::MemoryStorage,
};
use kernel::prelude::*;
#[kmain]
fn main() {
    test_main();
}
fn runtime() -> FixOnArca {
    runtime_with_storage(MemoryStorage::default())
}
fn runtime_with_storage<S: Storage + AsRef<MemoryStorage>>(storage: S) -> FixOnArca<S> {
    let author = Blob::canonical(stdlib::PRIMITIVES[0].1)
        .unwrap()
        .object()
        .into_ref();
    FixOnArca::new(storage).with_trust(Trust::parse(&format!("{}\n", author.as_handle())).unwrap())
}
fn program(storage: &dyn Storage, fixture: &str) -> Tree {
    test_program(storage, kernel::testing::fixture(fixture))
}
fn test_program(storage: &dyn Storage, bytes: &[u8]) -> Tree {
    let author = storage
        .add_blob(stdlib::PRIMITIVES[0].1)
        .object()
        .into_ref();
    storage
        .add_tree(&[
            author.into(),
            storage.add_blob(bytes).into(),
            Blob::literal(b"Runnable").unwrap().into(),
        ])
        .tag_descriptor()
}

#[kernel::test]
fn memory_limits_cover_startup_scratch_objects_and_descriptors() {
    let runtime = runtime();
    let storage = runtime.storage();
    let procedure = program(storage, "MEMORY_LIMITS");
    let run = |limit: u64, mode: u64, count: u64| {
        let number = |value: u64| storage.add_blob(&value.to_le_bytes()).into();
        let combination =
            storage.add_tree(&[number(limit), procedure.into(), number(mode), number(count)]);
        let fixruntime::arca::Execution::Complete(result) =
            runtime.run_observed(combination).unwrap()
        else {
            panic!("unexpected advice")
        };
        result
    };
    let failed = |value: Handle| {
        let fields = storage.get_tree(Tree::try_from(value).unwrap()).unwrap();
        assert_eq!(
            storage
                .get_blob(Blob::try_from(fields[0]).unwrap())
                .unwrap()
                .as_ref(),
            b"Fix.memory"
        );
    };
    let succeeded = |value: Handle| {
        assert_eq!(
            storage
                .get_blob(Blob::try_from(value).unwrap())
                .unwrap()
                .as_ref(),
            &42u64.to_le_bytes()
        );
    };
    failed(run(1, 0, 0));
    let limit = 64 << 20;
    succeeded(run(limit, 0, 320));
    failed(run(limit, 0, 640));
    succeeded(run(limit, 3, 320));
    succeeded(run(limit, 1, 1));
    failed(run(limit, 1, 64));
    succeeded(run(limit, 9, 128));
    failed(run(limit, 10, 128));
    succeeded(run(limit, 11, 128));
    succeeded(run(limit, 12, 128));
    succeeded(run(limit, 13, 128));
    succeeded(run(48 << 20, 2, 16));
    failed(run(48 << 20, 2, 1 << 17));
}

#[kernel::test]
fn resumed_applications_each_pay_for_their_captured_frame() {
    let memory = Arc::new(MemoryStorage::default());
    let runtime = runtime_with_storage(memory.clone());
    let storage = runtime.storage();
    let procedure = program(storage, "MEMORY_LIMITS");
    let number = |value: u64| storage.add_blob(&value.to_le_bytes()).into();
    let run = |combination| {
        let fixruntime::arca::Execution::Complete(result) =
            runtime.run_observed(combination).unwrap()
        else {
            panic!("unexpected advice");
        };
        result
    };
    let limit = number(64 << 20);
    let capture = storage.add_tree(&[limit, procedure.into(), number(5), number(320)]);
    let reservation =
        kernel::memory::Admission::new(kernel::memory::available() - (64 << 20)).unwrap();
    let available = kernel::memory::available();
    let result = run(capture);
    assert!(kernel::memory::available() <= available - (20 << 20));
    drop(reservation);
    let pair = storage.get_tree(Tree::try_from(result).unwrap()).unwrap();
    assert!(storage.equals(pair[0], number(42)).unwrap());
    let continuation = pair[1];
    let fields = storage
        .get_tree(Tree::try_from(continuation).unwrap())
        .unwrap();
    let checkpoint = TreeRef::try_from(fields[1]).unwrap().object_descriptor();

    for reply in [1, 2] {
        let resume = storage.add_tree(&[limit, continuation, number(reply)]);
        let result = run(resume);
        assert!(storage.equals(result, number(42)).unwrap());
        assert!(memory.present_children(checkpoint).is_none());
    }

    let resume = storage.add_tree(&[number(1 << 20), continuation, number(3)]);
    let result = run(resume);
    let error = storage.get_tree(Tree::try_from(result).unwrap()).unwrap();
    assert!(
        storage
            .equals(error[0], storage.add_blob(b"Fix.memory").into())
            .unwrap()
    );

    let wrong_arity = storage.add_tree(&[limit, continuation, number(1), number(2)]);
    let Err(error) = runtime.run_observed(wrong_arity) else {
        panic!("expected a rejected continuation application");
    };
    let report = error.request(storage, wrong_arity.into());
    let fields = storage.get_tree(report).unwrap();
    let message = storage.get_blob(Blob::try_from(fields[1]).unwrap()).unwrap();
    assert!(
        core::str::from_utf8(&message)
            .unwrap()
            .contains("tag kind \"Continuation\"")
    );
}

#[kernel::test]
fn sequential_implicit_resumes_reuse_the_application_budget() {
    use fixruntime::arca::Execution;

    let runtime = runtime();
    let storage = runtime.storage();
    let procedure = program(storage, "MEMORY_LIMITS");
    let number = |value: u64| storage.add_blob(&value.to_le_bytes()).into();
    let limit = number(64 << 20);
    let _reservation =
        kernel::memory::Admission::new(kernel::memory::available() - (160 << 20)).unwrap();
    for mode in [14, 15] {
        let mut application = storage.add_tree(&[limit, procedure.into(), number(mode), number(8)]);
        for _ in 0..8 {
            let Execution::Complete(result) = runtime.run_observed(application).unwrap() else {
                panic!("expected a captured continuation");
            };
            let fixruntime::ThunkView::Application(resume) =
                fixruntime::Thunk::try_from(result).unwrap().view()
            else {
                panic!("expected a continuation application");
            };
            application = resume.object_descriptor();
        }
        let Execution::Complete(result) = runtime.run_observed(application).unwrap() else {
            panic!("expected a completed continuation");
        };
        assert!(storage.equals(result, number(42)).unwrap());
    }
}

#[kernel::test]
fn stored_backing_stays_shared_across_capture() {
    let runtime = runtime();
    let storage = runtime.storage();
    let procedure = program(storage, "MEMORY_LIMITS");
    let number = |value: u64| storage.add_blob(&value.to_le_bytes()).into();
    let input = storage.add_blob(&vec![42; 80 << 20]);
    let limit = number(64 << 20);
    let run = |combination| {
        let fixruntime::arca::Execution::Complete(result) =
            runtime.run_observed(combination).unwrap()
        else {
            panic!("unexpected advice");
        };
        result
    };
    let capture = storage.add_tree(&[limit, procedure.into(), number(7), input.into()]);
    let result = run(capture);
    let pair = storage.get_tree(Tree::try_from(result).unwrap()).unwrap();
    let resume = storage.add_tree(&[limit, pair[1], number(0)]);
    assert!(storage.equals(run(resume), number(42)).unwrap());
    let persisted = storage.canonicalize(pair[1]).unwrap().into();
    let replay = storage.add_tree(&[limit, persisted, number(1)]);
    assert!(storage.equals(run(replay), number(42)).unwrap());

    let detach = storage.add_tree(&[limit, procedure.into(), number(8), input.into()]);
    let error = storage
        .get_tree(Tree::try_from(run(detach)).unwrap())
        .unwrap();
    assert!(
        storage
            .equals(error[0], storage.add_blob(b"Fix.memory").into())
            .unwrap()
    );
}

#[kernel::test]
fn checkpoint_page_handles_share_immutable_backing() {
    let storage = MemoryStorage::default();
    let mut page = Page::new(4096);
    assert_eq!(page.write(0, b"before"), 6);
    let mut page = Page::from_inner(page.into_inner().shared());
    let before = storage.add_page(page.clone());
    let reused = storage.add_page(page.clone());
    assert!(before.as_handle().same_encoding(reused.as_handle()));
    let mut duplicate = Page::new(4096);
    assert_eq!(duplicate.write(0, b"before"), 6);
    let duplicate = storage.add_page(duplicate);
    assert!(before.as_handle().same_encoding(duplicate.as_handle()));
    assert_eq!(storage.blob_bytes(), 4096);

    assert_eq!(page.write(0, b"after!"), 6);
    let after = storage.add_page(page);
    assert_eq!(&storage.get_blob(before).unwrap()[..6], b"before");
    assert_eq!(&storage.get_blob(after).unwrap()[..6], b"after!");
    assert_eq!(storage.blob_bytes(), 8192);
}

#[kernel::test]
fn call_cc_hands_off_the_application_reservation() {
    let runtime = runtime();
    let storage = runtime.storage();
    let procedure = program(storage, "MEMORY_LIMITS");
    let number = |value: u64| storage.add_blob(&value.to_le_bytes()).into();
    let application =
        storage.add_tree(&[number(1024 << 20), procedure.into(), number(6), number(0)]);
    let _reservation =
        kernel::memory::Admission::new(kernel::memory::available() - (1900 << 20)).unwrap();
    let fixruntime::arca::Execution::Complete(result) = runtime.run_observed(application).unwrap()
    else {
        panic!("unexpected advice");
    };
    let fixruntime::ThunkView::Application(resume) =
        fixruntime::Thunk::try_from(result).unwrap().view()
    else {
        panic!("expected a continuation application");
    };
    let fixruntime::arca::Execution::Complete(result) =
        runtime.run_observed(resume.object_descriptor()).unwrap()
    else {
        panic!("unexpected advice");
    };
    assert!(storage.equals(result, number(42)).unwrap());
}

#[kernel::test]
fn memory_admission_is_retryable_without_an_apply_coupon() {
    use fixruntime::ExecutionError;
    let runtime = runtime();
    let storage = runtime.storage();
    let procedure = program(storage, "IDENTITY");
    let limit = 256u64 << 20;
    let combination = storage.add_tree(&[
        storage.add_blob(&limit.to_le_bytes()).into(),
        procedure.into(),
    ]);
    let available = kernel::memory::available();
    let reservation = kernel::memory::Admission::new(available - (limit as usize / 2)).unwrap();
    assert!(matches!(
        runtime.execute(combination),
        Err(ExecutionError::Admission(_))
    ));
    assert!(
        runtime
            .cached_claims_in(
                &[
                    storage.add_blob(&2u64.to_le_bytes()).into(),
                    combination.into()
                ],
                &fixruntime::arca::SavedClaims::default()
            )
            .unwrap()
            .is_empty()
    );
    drop(reservation);
    assert!(runtime.execute(combination).is_ok());
    let missing = storage.add_tree(&[Blob::literal(b"").unwrap().into(), procedure.into()]);
    assert!(matches!(
        runtime.execute(missing),
        Err(ExecutionError::InvalidMemoryLimit)
    ));
}

#[kernel::test]
fn suspended_application_keeps_its_reservation() {
    use fixruntime::{ExecutionError, arca::Execution};
    let runtime = runtime();
    let storage = runtime.storage();
    let procedure = program(storage, "MEMORY_LIMITS");
    let number = |value: u64| storage.add_blob(&value.to_le_bytes()).into();
    let reserved =
        kernel::memory::Admission::new(kernel::memory::available() - (320 << 20)).unwrap();
    let application =
        storage.add_tree(&[number(256 << 20), procedure.into(), number(4), number(16)]);
    let Execution::Advice { event, suspended } = runtime.run_observed(application).unwrap() else {
        panic!("expected a live scratch allocation");
    };
    assert_eq!(event.key, b"memory::held");
    let privileged = storage.add_tree(&[number(1), procedure.into(), number(0), number(16)]);
    let result = runtime.execute_privileged(privileged).unwrap();
    assert!(storage.equals(result, number(42)).unwrap());
    let competing = storage.add_tree(&[number(256 << 20), procedure.into(), number(0), number(16)]);
    assert!(matches!(
        runtime.execute(competing),
        Err(ExecutionError::Admission(_))
    ));
    let Execution::Complete(result) = runtime.resume_observed(suspended).unwrap() else {
        panic!("expected completion");
    };
    assert_eq!(
        storage
            .get_blob(Blob::try_from(result).unwrap())
            .unwrap()
            .as_ref(),
        &42u64.to_le_bytes()
    );
    assert!(runtime.execute(competing).is_ok());
    drop(reserved);
}

#[kernel::test]
fn privileged_execution_can_exceed_its_admission_budget() {
    let runtime = runtime();
    let storage = runtime.storage();
    let procedure = program(storage, "MEMORY_LIMITS");
    let number = |value: u64| storage.add_blob(&value.to_le_bytes()).into();
    let combination = storage.add_tree(&[number(1), procedure.into(), number(0), number(16)]);
    let result = runtime.execute_privileged(combination).unwrap();
    assert_eq!(
        storage
            .get_blob(Blob::try_from(result).unwrap())
            .unwrap()
            .as_ref(),
        &42u64.to_le_bytes()
    );
}
#[kernel::test]
fn scalar_descriptor_identity_and_c_bindings() {
    let runtime = runtime();
    let storage = runtime.storage();
    let identity = program(storage, "IDENTITY");
    let combination = storage.add_tree(&[
        Blob::literal(&(512u64 << 20).to_le_bytes()).unwrap().into(),
        identity.into(),
        storage.add_blob(&42u64.to_le_bytes()).into(),
    ]);
    let result = runtime.execute_privileged(combination).unwrap();
    assert!(storage.equals(result, combination.into()).unwrap());
    let bindings = program(storage, "BINDINGS");
    let combination = storage.add_tree(&[
        Blob::literal(&(512u64 << 20).to_le_bytes()).unwrap().into(),
        bindings.into(),
    ]);
    let result = runtime.execute_privileged(combination).unwrap();
    let bytes = storage.get_blob(Blob::try_from(result).unwrap()).unwrap();
    assert_eq!(bytes.as_ref(), 42u64.to_le_bytes());
}
#[kernel::test]
fn descriptor_slots_survive_capture_and_serialized_replay() {
    let runtime = runtime();
    let storage = runtime.storage();
    let procedure = program(storage, "CAPTURE");
    let input = storage.add_blob(&[42; 64]);
    let request = storage.add_tree(&[Blob::literal(b"capture").unwrap().into()]);
    let combination = storage.add_tree(&[
        Blob::literal(&(512u64 << 20).to_le_bytes()).unwrap().into(),
        procedure.into(),
        input.into(),
        request.into(),
        storage.add_blob(&0u64.to_le_bytes()).into(),
    ]);
    let mut result = runtime.execute_privileged(combination).unwrap();
    for _ in 0..2 {
        let pair = storage.get_tree(Tree::try_from(result).unwrap()).unwrap();
        assert!(storage.equals(pair[0], request.into()).unwrap());
        let continuation: Handle = storage.canonicalize(pair[1]).unwrap().into();
        let resumed = storage.add_tree(&[
            Blob::literal(&(512u64 << 20).to_le_bytes()).unwrap().into(),
            continuation,
            Blob::literal(b"reply").unwrap().into(),
        ]);
        result = runtime.execute_privileged(resumed).unwrap();
    }
    assert!(storage.equals(result, input.into()).unwrap());
}

#[kernel::test]
fn eager_parser_returns_values_and_applications() {
    let runtime = runtime();
    let storage = runtime.storage();
    let parser = program(storage, "PARSER");
    let environment = stdlib::build_environment(storage);
    let combination = storage.add_tree(&[
        Blob::literal(&(512u64 << 20).to_le_bytes()).unwrap().into(),
        parser.into(),
        storage.add_blob(b"42u64").into(),
        environment,
    ]);
    let result = runtime.execute_privileged(combination).unwrap();
    let parsed = storage.get_tree(Tree::try_from(result).unwrap()).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(
        storage
            .get_blob(Blob::try_from(parsed[0]).unwrap())
            .unwrap()
            .as_ref(),
        42u64.to_le_bytes()
    );
    let combination = storage.add_tree(&[
        Blob::literal(&(512u64 << 20).to_le_bytes()).unwrap().into(),
        parser.into(),
        storage.add_blob(b"*#(268435456u64 $identity 42u64)").into(),
        environment,
    ]);
    let result = runtime.execute_privileged(combination).unwrap();
    let parsed = storage.get_tree(Tree::try_from(result).unwrap()).unwrap();
    assert_eq!(parsed.len(), 1);
    assert!(matches!(
        parsed[0].view(),
        fixruntime::HandleView::Encode(_)
    ));
}

#[path = "evaluation.rs"]
mod evaluation;

#[kernel::test]
fn eager_scheduler_evaluates_a_literal() {
    let evaluation = evaluation::Evaluation::new(runtime());
    let storage = evaluation.storage();
    let literal = storage.add_blob(&42u64.to_le_bytes());
    let result = evaluation.eval(literal.into());
    assert!(storage.equals(result, literal.into()).unwrap());
}

#[kernel::test]
fn eager_scheduler_evaluates_selection() {
    let evaluation = evaluation::Evaluation::new(runtime());
    let storage = evaluation.storage();
    let literal = storage.add_blob(&42u64.to_le_bytes());
    let values = storage.add_tree(&[literal.into()]);
    let arguments = storage.add_tree(&[
        values.into(),
        Blob::literal(&0u64.to_le_bytes()).unwrap().into(),
    ]);
    let result = evaluation.eval(arguments.into_ref().selection().strict().into());
    assert!(storage.equals(result, literal.into()).unwrap());
}

#[kernel::test]
fn memories_and_tables_share_the_lower_64_regions() {
    let runtime = runtime();
    let storage = runtime.storage();
    for fixture in ["MANY_MEMORIES", "MANY_TABLES"] {
        let procedure = program(storage, fixture);
        let combination = storage.add_tree(&[
            Blob::literal(&(512u64 << 20).to_le_bytes()).unwrap().into(),
            procedure.into(),
        ]);
        let result = runtime.execute_privileged(combination).unwrap();
        let fields = storage.get_tree(Tree::try_from(result).unwrap()).unwrap();
        assert_eq!(fields.len(), 2);
        let bytes = storage
            .get_blob(Blob::try_from(fields[0]).unwrap())
            .unwrap();
        let mut expected = [0; 64];
        expected[0] = 43;
        assert_eq!(&*bytes, &expected);
        let number = storage
            .get_blob(Blob::try_from(fields[1]).unwrap())
            .unwrap();
        assert_eq!(&*number, &44u64.to_le_bytes());
    }
}

#[kernel::test]
fn memory_access_cannot_span_adjacent_regions() {
    let mut function: Function =
        common::elfloader::load_elf(kernel::testing::fixture("MEMORY_BOUNDARY")).unwrap();
    for address in [0xfffff000, 0x7e00000000] {
        assert!(
            function
                .inner_mut()
                .arca_mut()
                .unwrap()
                .mappings_mut()
                .map(address, Entry::RWPage(Page::new(4096)))
                .is_ok()
        );
    }
    let effect: Function = function
        .apply(kernel::prelude::Blob::new(0x7e00000000u64.to_le_bytes()))
        .force()
        .try_into()
        .unwrap();
    let (name, arguments) = effect.into_inner().into_symbolic().unwrap();
    let name: kernel::prelude::Blob = name.try_into().unwrap();
    assert_eq!(&*name, b"trap");
    let reason: kernel::prelude::Word = arguments.front().unwrap().clone().try_into().unwrap();
    assert_eq!(reason.read(), 1);
}

#[kernel::test]
fn privileged_tree_bytes_remain_attached_after_closing_the_descriptor() {
    let runtime = runtime();
    let storage = runtime.storage();
    let procedure = program(storage, "TREE_BYTES");
    let combination = storage.add_tree(&[
        Blob::literal(&(512u64 << 20).to_le_bytes()).unwrap().into(),
        procedure.into(),
    ]);
    let result = runtime.execute_privileged(combination).unwrap();
    let bytes = storage.get_blob(Blob::try_from(result).unwrap()).unwrap();
    assert_eq!(&*bytes, &85u64.to_le_bytes());
}

#[kernel::test]
fn invalid_tree_reads_return_deterministic_traps() {
    let runtime = runtime();
    let storage = runtime.storage();
    let read = program(storage, "TREE_READ");
    let add = program(storage, "ADD_BLOB");
    let metadata: Handle = Blob::literal(&(512u64 << 20).to_le_bytes()).unwrap().into();
    let number: Handle = storage.add_blob(&42u64.to_le_bytes()).into();
    let empty = storage.add_tree(&[]);
    for (request, code) in [
        (storage.add_tree(&[metadata, add.into()]), 1u32),
        (storage.add_tree(&[metadata, add.into(), number]), 1),
        (storage.add_tree(&[metadata, read.into(), number]), 5),
        (storage.add_tree(&[metadata, read.into(), empty.into()]), 1),
        (
            storage.add_tree(&[metadata, read.into(), empty.into_ref().into()]),
            5,
        ),
    ] {
        let result = runtime.execute_privileged(request).unwrap();
        let repeated = runtime.execute_privileged(request).unwrap();
        assert!(storage.equals(result, repeated).unwrap());
        let result = Tree::try_from(result).unwrap();
        assert!(!result.is_tag());
        let fields = storage.get_tree(result).unwrap();
        assert_eq!(fields.len(), 2);
        let marker = storage
            .get_blob(Blob::try_from(fields[0]).unwrap())
            .unwrap();
        assert_eq!(marker.as_ref(), b"Wasm.trap");
        let reason = storage
            .get_blob(Blob::try_from(fields[1]).unwrap())
            .unwrap();
        assert_eq!(reason.as_ref(), code.to_le_bytes());
    }
    let child = storage.add_tree(&[number]);
    let request = storage.add_tree(&[metadata, read.into(), child.into()]);
    assert!(
        storage
            .equals(runtime.execute_privileged(request).unwrap(), number)
            .unwrap()
    );
    let request = storage.add_tree(&[metadata, add.into(), number, number]);
    let result = runtime.execute_privileged(request).unwrap();
    let result = storage.get_blob(Blob::try_from(result).unwrap()).unwrap();
    assert_eq!(result.as_ref(), 84u64.to_le_bytes());
}
