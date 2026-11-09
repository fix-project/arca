use super::{test_program, test_runtime};
use fix::{Blob, Handle, Runtime, Tree, user_scheduler_driver};
use kernel::prelude::{Arc, Vec, vec};

#[kernel::test]
fn shared_maps_compare_handle_bytes_and_update_atomically() {
    let runtime = Arc::new(test_runtime());
    let storage = runtime.storage();
    let blob = storage.add_blob(&vec![42; 8193]);
    let leaf = storage.add_tree(&[blob.into(), storage.add_blob(b"leaf").into()]);
    let tree = storage.add_tree(&[leaf.into(), leaf.into()]);
    let tag = storage
        .add_tree(&[storage.add_blob(b"author").into(), leaf.into()])
        .tag_descriptor();
    let keys: Vec<Handle> = [
        storage.add_blob(b"literal").into(),
        blob.into(),
        leaf.into(),
        tree.into(),
        tag.into(),
        blob.object().into_ref().into(),
        tree.into_ref().into(),
    ]
    .into_iter()
    .map(|key| {
        let canonical: Handle = storage.canonicalize(key).unwrap().into();
        storage.add_tree(&[key, canonical]).into()
    })
    .collect();
    let program = test_program(storage, kernel::testing::fixture("FIX_CONCURRENT_MAP"));
    let combination = storage.add_tree(&[
        Blob::literal(b"").unwrap().into(),
        program.into(),
        storage.add_tree(&keys).into(),
        storage
            .add_tree(&[
                storage.add_blob(b"counter").into(),
                storage.add_blob(&[42; 40]).into(),
            ])
            .into(),
    ]);
    let report = user_scheduler_driver::start(
        runtime.clone(),
        combination,
        4,
        user_scheduler_driver::PersistenceOnly(Ok),
    )
    .unwrap();
    let result = storage
        .get_tree(Tree::try_from(report.result.unwrap()).unwrap())
        .unwrap();
    assert_eq!(
        storage
            .get_blob(Blob::try_from(result[0]).unwrap())
            .unwrap()
            .as_ref(),
        &256u64.to_le_bytes()
    );
}
