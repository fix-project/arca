use super::test_program;
use fix::{
    Blob, Handle, Storage, Tree, api::Backend, arca::trust::Trust, repository::Repository,
    storage::repository::RepositoryStorage,
};
use fixapi::Memoization;
use kernel::prelude::{Arc, Vec, format};

#[kernel::test]
fn backend_executes_with_the_configured_authorities() {
    let storage = Arc::new(RepositoryStorage::new(Repository::init(".fix").unwrap()));
    let identity = test_program(&*storage, fix::stdlib::PRIMITIVES[0].1);
    let scheduler = test_program(&*storage, kernel::testing::fixture("FIX_SCHEDULER"));
    let collector = test_program(&*storage, kernel::testing::fixture("FIX_COLLECTOR"));
    let author = storage
        .add_blob(fix::stdlib::PRIMITIVES[0].1)
        .object()
        .into_ref();
    let author: Handle = storage.canonicalize(author.into()).unwrap().into();
    let mut trust = Trust::parse(&format!("{author}\n")).unwrap();
    trust.add_tagger(&*storage, collector).unwrap();
    let backend = Backend::new(storage, trust, Memoization::Auto);
    let argument = backend.blob(&[42; 64]);
    let combination = backend.tree(&[
        Blob::literal(b"").unwrap().into(),
        identity.into(),
        argument,
    ]);
    let combination = Tree::try_from(combination).unwrap();
    let result = backend.call(combination).unwrap();
    assert!(
        backend
            .storage()
            .equals(result, combination.into())
            .unwrap()
    );
    let request: Handle = combination.into_ref().application().strict().into();
    let result = backend
        .eval(request, scheduler, collector)
        .unwrap()
        .unwrap();
    assert!(
        backend
            .storage()
            .equals(result, combination.into())
            .unwrap()
    );
}

fn data_backend() -> Backend {
    let storage = Arc::new(RepositoryStorage::new(Repository::init(".fix").unwrap()));
    Backend::new(storage, Trust::default(), Memoization::Never)
}

#[kernel::test]
fn backend_reads_blob_slices_and_rejects_out_of_bounds_reads() {
    let backend = data_backend();
    let contents: Vec<_> = (0..12289).map(|index| (index % 251) as u8).collect();
    let blob = backend.blob(&contents);
    let mut bytes = [0; 8201];
    backend.read(blob, 4088, &mut bytes).unwrap();
    assert_eq!(bytes.as_slice(), &contents[4088..]);
    assert!(backend.read(blob, contents.len(), &mut bytes).is_err());
}

#[kernel::test]
fn backend_labels_preserve_tree_contents() {
    let backend = data_backend();
    let blob = backend.blob(&[42; 64]);
    let tree = backend.tree(&[blob]);
    backend.label("answer", tree).unwrap();
    let saved = backend.lookup("answer").unwrap();
    assert!(saved.is_canonical());
    let fields = backend.read_tree(saved).unwrap();
    assert_eq!(fields.len(), 1);
    assert_eq!(backend.read_blob(fields[0]).unwrap().as_ref(), &[42; 64]);
}
