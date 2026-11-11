#![no_main]
#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(kernel::testing::harness)]
#![reexport_test_harness_main = "test_main"]
use fixruntime::{
    Blob, Handle, Storage, Tree,
    arca::{FixOnArca, trust::Trust},
    runtime::Runtime,
    stdlib,
};
use kernel::prelude::*;
#[kmain]
fn main() {
    test_main();
}
fn runtime() -> FixOnArca {
    let author = Blob::canonical(stdlib::PRIMITIVES[0].1)
        .unwrap()
        .object()
        .into_ref();
    FixOnArca::default().with_trust(Trust::parse(&format!("{}\n", author.as_handle())).unwrap())
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
fn scalar_descriptor_identity_and_c_bindings() {
    let runtime = runtime();
    let storage = runtime.storage();
    let identity = program(storage, "IDENTITY");
    let combination = storage.add_tree(&[
        Blob::literal(b"").unwrap().into(),
        identity.into(),
        storage.add_blob(&42u64.to_le_bytes()).into(),
    ]);
    let result = runtime.execute_privileged(combination).unwrap();
    assert!(storage.equals(result, combination.into()).unwrap());
    let bindings = program(storage, "BINDINGS");
    let combination = storage.add_tree(&[Blob::literal(b"").unwrap().into(), bindings.into()]);
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
        Blob::literal(b"").unwrap().into(),
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
            Blob::literal(b"").unwrap().into(),
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
        Blob::literal(b"").unwrap().into(),
        parser.into(),
        storage.add_blob(b"42u64").into(),
        environment,
    ]);
    let result = runtime.execute_privileged(combination).unwrap();
    assert_eq!(
        storage
            .get_blob(Blob::try_from(result).unwrap())
            .unwrap()
            .as_ref(),
        42u64.to_le_bytes()
    );
    let combination = storage.add_tree(&[
        Blob::literal(b"").unwrap().into(),
        parser.into(),
        storage.add_blob(b"*#(\"\" $identity 42u64)").into(),
        environment,
    ]);
    let result = runtime.execute_privileged(combination).unwrap();
    assert!(matches!(result.view(), fixruntime::HandleView::Encode(_)));
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
        let combination = storage.add_tree(&[Blob::literal(b"").unwrap().into(), procedure.into()]);
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
    let combination = storage.add_tree(&[Blob::literal(b"").unwrap().into(), procedure.into()]);
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
    let metadata: Handle = Blob::literal(b"").unwrap().into();
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
