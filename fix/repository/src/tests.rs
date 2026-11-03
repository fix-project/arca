use super::*;
use std::path::PathBuf;

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::var_os("TEST_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let path = root.join(format!(
            "repository-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn repository(&self) -> Repository {
        Repository::init(self.0.join(".fix").to_str().unwrap()).unwrap()
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn empty_trees_round_trip_through_a_reopened_repository() {
    let directory = Directory::new();
    let repository = directory.repository();
    let empty = repository.create_tree(&[]).unwrap();
    assert!(empty.as_handle().same_encoding(&Tree::empty().into()));
    let reopened = Repository::open(repository.path()).unwrap();
    assert!(reopened.has_backing(empty.object()).unwrap());
    assert!(reopened.read_tree(empty).unwrap().is_empty());
    let reference: Handle = empty.into_ref().into();
    assert!(
        reopened
            .lookup(reference.as_bytes())
            .unwrap()
            .same_encoding(&reference)
    );
    let parent = reopened.create_tree(&[empty.into(), reference]).unwrap();
    let fields = reopened.read_tree(parent).unwrap();
    assert!(fields[0].same_encoding(&empty.into()));
    assert!(fields[1].same_encoding(&reference));
    let forged = unsafe { Tree::named(Name::Canonical(&[0; 24]), 0, true, true).unwrap() };
    assert!(reopened.read_tree(forged).is_err());
}

#[test]
fn persistent_objects_preserve_the_standard_handle_format() {
    let directory = Directory::new();
    let repository = directory.repository();
    let data: Vec<_> = (0..12289).map(|index| (index % 251) as u8).collect();
    let blob = repository.create_blob(&data).unwrap();
    assert_eq!(
        blob.handle().as_bytes(),
        Blob::canonical(&data).unwrap().handle().as_bytes()
    );
    assert_eq!(repository.read_blob(blob).unwrap().as_ref(), data);
    let literal = repository.create_blob(b"literal").unwrap();
    let fields = [literal.into(), blob.into_ref().into()];
    let tree = repository.create_tree(&fields).unwrap();
    assert_eq!(
        tree.handle().as_bytes(),
        Tree::canonical(&fields).unwrap().handle().as_bytes()
    );
    repository.write_label("root", tree.into()).unwrap();
    let reopened = Repository::open(repository.path()).unwrap();
    assert_eq!(
        reopened.read_label("root").unwrap().as_bytes(),
        tree.handle().as_bytes()
    );
    assert!(
        reopened
            .read_tree(tree)
            .unwrap()
            .iter()
            .zip(fields)
            .all(|(a, b)| a.same_encoding(&b))
    );
    assert_eq!(reopened.labels().unwrap(), ["root"]);
}

#[test]
fn thin_references_retain_their_todo_metadata() {
    let directory = Directory::new();
    let repository = directory.repository();
    let object = repository.create_blob(&[42; 64]).unwrap();
    let reference = object.object().into_ref();
    let uri = repository
        .create_blob(b"file:///source/package.tar")
        .unwrap();
    let metadata = repository.create_tree(&[uri.into()]).unwrap();
    repository.set_todo(reference, metadata.into()).unwrap();
    std::fs::remove_file(
        directory
            .0
            .join(".fix/data")
            .join(Repository::data_key(object.handle().as_bytes())),
    )
    .unwrap();
    assert!(!repository.has_backing(object.object()).unwrap());
    assert!(
        repository
            .lookup(reference.handle().as_bytes())
            .unwrap()
            .same_encoding(&reference.into())
    );
    let hints = repository.get_todo(reference).unwrap().unwrap();
    assert!(hints.same_encoding(&metadata.into()));
    assert!(
        repository
            .read_tree(Tree::try_from(hints).unwrap())
            .unwrap()[0]
            .same_encoding(&uri.into())
    );
}

#[test]
fn coupon_indexes_preserve_author_and_claim_fields() {
    let directory = Directory::new();
    let repository = directory.repository();
    let author = repository.create_blob(b"collector").unwrap();
    let statement = repository.create_blob(&1u64.to_le_bytes()).unwrap();
    let input = repository.create_blob(b"input").unwrap();
    let output = repository.create_blob(b"output").unwrap();
    let claim = repository
        .create_tree(&[statement.into(), input.into(), output.into()])
        .unwrap();
    let fields = [author.into(), claim.into_ref().into()];
    let coupon = Tree::canonical(&fields).unwrap().tag_descriptor();
    let bytes: Vec<_> = fields.iter().flat_map(|field| *field.as_bytes()).collect();
    repository.publish(coupon.into(), &bytes).unwrap();
    repository.publish(coupon.into(), &bytes).unwrap();
    let claims = repository
        .claims(&[statement.into(), input.into()])
        .unwrap();
    assert!(
        claims
            .iter()
            .any(|claim| claim.handle().same_encoding(&coupon.handle()))
    );
    assert!(repository.read_tree(coupon).unwrap()[0].same_encoding(&author.into()));
}

#[test]
fn labels_are_atomic_and_repository_discovery_is_relative() {
    let directory = Directory::new();
    let repository = directory.repository();
    let nested = directory.0.join("source/nested");
    std::fs::create_dir_all(&nested).unwrap();
    let discovered = Repository::discover(nested.to_str().unwrap()).unwrap();
    assert_eq!(discovered.path(), repository.path());
    let values = [
        repository.create_blob(b"first").unwrap(),
        repository.create_blob(b"second").unwrap(),
    ];
    repository.write_label("value", values[0].into()).unwrap();
    std::thread::scope(|scope| {
        for value in values {
            let repository = &repository;
            scope.spawn(move || {
                for _ in 0..32 {
                    repository.write_label("value", value.into()).unwrap();
                }
            });
        }
        for _ in 0..64 {
            let value = repository.read_label("value").unwrap();
            assert!(
                values
                    .iter()
                    .any(|expected| expected.handle().same_encoding(&value))
            );
        }
    });
    assert!(matches!(
        repository.write_label("../outside", values[0].into()),
        Err(RepositoryError::InvalidLabel)
    ));
}
