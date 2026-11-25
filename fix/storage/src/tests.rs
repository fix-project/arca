use super::*;
use alloc::format;
use memory::MemoryStorage;

#[test]
fn empty_trees_are_available_across_stores_and_repositories() {
    let storage = MemoryStorage::default();
    let empty = storage.add_tree(&[]);
    assert!(empty.as_handle().same_encoding(&Tree::empty().into()));
    let other = MemoryStorage::default();
    assert!(other.get_tree(empty).unwrap().is_empty());
    assert!(
        other
            .identify(empty.as_handle().as_bytes())
            .unwrap()
            .same_encoding(&empty.into())
    );
    let parent = storage.add_tree(&[empty.into_ref().into(), empty.into()]);
    let canonical: Handle = storage.canonicalize(parent.into()).unwrap().into();
    let expected = Tree::canonical(&[empty.into_ref().into(), empty.into()]).unwrap();
    assert!(canonical.same_encoding(&expected.into()));
}

#[test]
fn saved_values_are_readable_after_reopening() {
    let path = std::env::var_os("TEST_TMPDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("storage-repository-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let repository = fixrepository::Repository::init(path.join(".fix").to_str().unwrap()).unwrap();
    let storage = repository::RepositoryStorage::new(repository);
    let blob = storage.add_blob(&[7; 64]);
    let tree = storage.add_tree(&[blob.into()]);
    let canonical = storage.save(tree.into()).unwrap();
    assert!(canonical.is_canonical());
    assert!(storage.save(tree.into()).unwrap().same_encoding(&canonical));
    drop(storage);
    let reopened = fixrepository::Repository::open(path.join(".fix").to_str().unwrap()).unwrap();
    let saved = reopened
        .read_tree(Tree::try_from(canonical).unwrap())
        .unwrap();
    assert!(saved[0].is_canonical());
    assert_eq!(
        reopened
            .read_blob(Blob::try_from(saved[0]).unwrap())
            .unwrap()
            .as_ref(),
        &[7; 64]
    );
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn a_repository_can_read_from_an_implicit_storage_source() {
    let path = std::env::var_os("TEST_TMPDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("storage-source-{}", std::process::id()));
    std::fs::create_dir_all(path.join("user")).unwrap();
    std::fs::create_dir(path.join("project")).unwrap();
    let source = fixrepository::Repository::init(path.join("user/.fix").to_str().unwrap()).unwrap();
    let blob = source.create_blob(&[7; 128]).unwrap();
    let project =
        fixrepository::Repository::init(path.join("project/.fix").to_str().unwrap()).unwrap();
    let storage = repository::RepositoryStorage::new(project).with_source(source);
    assert!(storage.identify(blob.as_handle().as_bytes()).is_ok());
    assert!(storage.has_blob(blob));
    assert_eq!(storage.get_blob(blob).unwrap().as_ref(), &[7; 128]);
    assert!(!storage.repository().has_backing(blob.object()).unwrap());
    storage.save(blob.into()).unwrap();
    assert!(storage.repository().has_backing(blob.object()).unwrap());
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn canonicalization_preserves_deep_graphs() {
    let storage = MemoryStorage::default();
    let mut machine: Handle = Blob::literal(b"leaf").unwrap().into();
    let mut expected = machine;
    for _ in 0..4096 {
        machine = storage.add_tree(&[machine]).into_ref().selection().into();
        expected = Tree::canonical(&[expected])
            .unwrap()
            .into_ref()
            .selection()
            .into();
    }
    let canonical: Handle = storage.canonicalize(machine).unwrap().into();
    assert!(canonical.same_encoding(&expected));
    let repeated: Handle = storage.canonicalize(machine).unwrap().into();
    assert!(repeated.same_encoding(&canonical));
}

#[test]
fn loading_preserves_direct_and_referenced_objects() {
    let path = std::env::var_os("TEST_TMPDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("storage-load-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let repository = path.join(".fix");
    let repository = repository.to_str().unwrap();
    let saved = {
        let storage = repository::RepositoryStorage::new(
            fixrepository::Repository::init(repository).unwrap(),
        );
        let direct = storage.add_blob(&[1; 64]);
        let referenced = storage.add_blob(&[2; 64]);
        let tree = storage.add_tree(&[direct.into(), referenced.into_ref().into()]);
        storage.save(tree.into()).unwrap()
    };
    let storage =
        repository::RepositoryStorage::new(fixrepository::Repository::open(repository).unwrap());
    let fields = storage
        .repository()
        .read_tree(Tree::try_from(saved).unwrap())
        .unwrap();
    storage.load(saved);
    assert_eq!(
        storage
            .get_blob(Blob::try_from(fields[0]).unwrap())
            .unwrap()
            .as_ref(),
        &[1; 64]
    );
    let referenced: Handle = Ref::try_from(fields[1]).unwrap().object_descriptor().into();
    assert_eq!(
        storage
            .get_blob(Blob::try_from(referenced).unwrap())
            .unwrap()
            .as_ref(),
        &[2; 64]
    );
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn loading_retries_newly_published_descendants() {
    let path =
        std::path::PathBuf::from(std::env::var_os("TEST_TMPDIR").unwrap()).join("residency-retry");
    std::fs::create_dir(&path).unwrap();
    let repository = fixrepository::Repository::init(path.join(".fix").to_str().unwrap()).unwrap();
    let contents = [7; 64];
    let child = Blob::canonical(&contents).unwrap();
    let canonical = Tree::canonical(&[child.into()]).unwrap();
    repository
        .publish(canonical.into(), child.as_handle().as_bytes())
        .unwrap();
    let storage = repository::RepositoryStorage::new(repository);
    let parent = storage.add_tree(&[canonical.into(), canonical.into()]);
    storage.load(parent.into());
    storage
        .repository()
        .publish(child.into(), &contents)
        .unwrap();
    storage.load(parent.into());
    assert_eq!(storage.get_blob(child).unwrap().as_ref(), &contents);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn friendly_names_use_labels_and_fix_notation_without_materializing_data() {
    let storage = MemoryStorage::default();
    let module = Blob::canonical(&[41; 64]).unwrap();
    let reference = module.into_ref();
    let labels = [(
        *reference.as_handle().as_bytes(),
        alloc::string::String::from("module"),
    )];
    let input: Handle = reference.identification().strict().into();
    assert_eq!(storage.friendly_name(input, &labels), "*'module");
    assert_eq!(storage.friendly_name(module.into(), &labels), "*'module");
    let selection = storage
        .add_tree(&[
            reference.into(),
            Blob::literal(&0u64.to_le_bytes()).unwrap().into(),
        ])
        .into_ref()
        .selection();
    assert_eq!(
        storage.friendly_name(selection.into(), &labels),
        "[module 0u64]"
    );
    assert_eq!(
        storage.friendly_name(selection.strict().into(), &labels),
        "*[module 0u64]"
    );
    let object_labels = [(
        *module.as_handle().as_bytes(),
        alloc::string::String::from("bytes"),
    )];
    assert_eq!(
        storage.friendly_name(reference.into(), &object_labels),
        "&bytes"
    );
    assert_eq!(storage.friendly_name(input, &object_labels), "*'bytes");
    let coupon = storage.certificate(1, input, reference.into());
    let coupon: Handle = storage.canonicalize(coupon.into()).unwrap().into();
    assert_eq!(
        storage.friendly_name(coupon, &labels),
        "Eval(*'module, module)"
    );
    assert_eq!(
        storage.friendly_name(coupon.object_descriptor().into_ref().into(), &labels),
        "&Eval(*'module, module)"
    );
}

#[test]
fn friendly_coupons_describe_either_named_side() {
    let storage = MemoryStorage::default();
    let named = Blob::canonical(&[53; 64]).unwrap().into_ref();
    let other = Blob::canonical(&[54; 64]).unwrap().into_ref();
    let labels = [(
        *named.as_handle().as_bytes(),
        alloc::string::String::from("module"),
    )];
    let input_named = storage.certificate(1, named.into(), other.into());
    let result_named = storage.certificate(1, other.into(), named.into());
    let input_named: Handle = storage.canonicalize(input_named.into()).unwrap().into();
    let result_named: Handle = storage.canonicalize(result_named.into()).unwrap().into();
    let other = storage.friendly_name(other.into(), &labels);
    assert_eq!(
        storage.friendly_name(input_named, &labels),
        format!("Eval(module, {other})")
    );
    assert_eq!(
        storage.friendly_name(result_named, &labels),
        format!("Eval({other}, module)")
    );
}

#[test]
fn friendly_canonical_prefixes_distinguish_known_names_and_preserve_type() {
    let storage = MemoryStorage::default();
    let module = Blob::canonical(&[42; 64]).unwrap();
    let mut neighbor = *module.as_handle().as_bytes();
    neighbor[4] ^= 0x10;
    storage.register_names([neighbor]);
    let full = format!("{}", module.as_handle());
    assert_eq!(
        storage.friendly_name(module.into(), &[]),
        format!("{}.80", &full[..9])
    );
    assert_eq!(
        storage.friendly_name(module.into_ref().into(), &[]),
        format!("{}.90", &full[..9])
    );
}

fn repository_at(name: &str) -> (std::path::PathBuf, fixrepository::Repository) {
    let path = std::env::var_os("TEST_TMPDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("{name}-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let repository = fixrepository::Repository::init(path.join(".fix").to_str().unwrap()).unwrap();
    (path, repository)
}

#[test]
fn eviction_releases_ram_and_load_restores_tree_descendants() {
    let (path, repository) = repository_at("storage-eviction");
    let contents = [42; 1024];
    let blob = repository.create_blob(&contents).unwrap();
    let tree = repository.create_tree(&[blob.into()]).unwrap();
    let storage = repository::RepositoryStorage::new(repository);
    let memory: &MemoryStorage = storage.as_ref();
    storage.load(tree.into());
    let read = memory.get_blob_backing(blob).unwrap();
    assert!(memory.evict(blob));
    assert!(!memory.evict(blob));
    assert_eq!(memory.blob_bytes(), 0);
    assert_eq!(read.to_bytes().as_ref(), &contents);
    assert!(memory.get_blob(blob).is_err());
    assert_eq!(
        storage.repository().read_blob(blob).unwrap().as_ref(),
        &contents
    );
    storage.load(tree.into());
    assert_eq!(memory.get_blob(blob).unwrap().as_ref(), &contents);
    assert!(memory.evict(blob));
    assert_eq!(storage.get_blob(blob).unwrap().as_ref(), &contents);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn object_edges_keep_blobs_resident_until_the_last_scope_ends() {
    let memory = MemoryStorage::default();
    let blob = memory.add_blob(&[1; 64]);
    let lazy = memory.add_blob(&[2; 64]);
    let canonical = memory.canonicalize_blob(blob).unwrap();
    let child = memory.add_tree(&[blob.into(), canonical.into()]);
    let root = memory.add_tree(&[child.into(), lazy.object().into_ref().into()]);

    let mut application = memory.blob_pins();
    let mut scheduler = memory.blob_pins();
    application.retain(&memory, root.into()).unwrap();
    let borrowed = application.get(blob).unwrap();
    assert!(matches!(&borrowed, alloc::borrow::Cow::Borrowed(_)));
    assert_eq!(borrowed.to_bytes().as_ref(), &[1; 64]);
    drop(borrowed);
    scheduler.retain(&memory, canonical.into()).unwrap();
    scheduler
        .release_unreachable(&memory, blob, |_| {
            panic!("another invocation pins this blob")
        })
        .unwrap();
    assert!(memory.evict(lazy));
    assert!(!memory.evict(blob));
    drop(application);
    assert!(!memory.evict(blob));
    assert_eq!(memory.get_blob(blob).unwrap().as_ref(), &[1; 64]);
    drop(scheduler);
    assert!(memory.evict(blob));
}

#[test]
fn releasing_an_unreachable_blob_preserves_other_pins_and_allows_repinning() {
    let memory = MemoryStorage::default();
    let blob = memory.add_blob(&[3; 64]);
    let other = memory.add_blob(&[4; 64]);
    let tree = memory.add_tree(&[blob.into(), other.into()]);
    let mut pins = memory.blob_pins();
    pins.retain(&memory, tree.into()).unwrap();
    pins.release_unreachable(&memory, blob, |matches| matches(tree.into()))
        .unwrap();
    assert!(!memory.evict(blob));
    pins.release_unreachable(&memory, blob, |matches| matches(other.into()))
        .unwrap();
    assert!(pins.get(blob).is_err());
    assert!(!memory.evict(other));
    pins.retain(&memory, tree.into()).unwrap();
    assert!(!memory.evict(blob));
    drop(pins);
    assert!(memory.evict(blob));
}

#[test]
fn a_live_duplicate_can_republish_an_evicted_canonical_blob() {
    let memory = MemoryStorage::default();
    let contents = [7; 64];
    let first = memory.add_blob(&contents);
    let second = memory.add_blob(&contents);
    let canonical = Blob::try_from(Handle::from(memory.canonicalize_blob(first).unwrap())).unwrap();
    memory.canonicalize_blob(second).unwrap();
    assert!(memory.evict(first));
    assert!(memory.get_blob(canonical).is_err());
    assert!(memory.get_blob(first).is_err());
    memory.canonicalize_blob(second).unwrap();
    assert_eq!(memory.get_blob(canonical).unwrap().as_ref(), &contents);
    assert!(!memory.evict(first));
    assert_eq!(memory.get_blob(second).unwrap().as_ref(), &contents);
    assert!(memory.evict(canonical));
    assert_eq!(memory.blob_bytes(), 0);
    assert!(!memory.evict(Blob::literal(b"inline").unwrap()));
}

#[test]
fn concurrent_reads_and_reloads_return_the_named_bytes() {
    use std::sync::Barrier;

    let memory = MemoryStorage::default();
    let bytes = [93; 64];
    let machine = memory.add_blob(&bytes);
    let canonical =
        Blob::try_from(Handle::from(memory.canonicalize_blob(machine).unwrap())).unwrap();
    let retained = memory.get_blob_backing(machine).unwrap();
    let start = Barrier::new(5);
    std::thread::scope(|threads| {
        for name in [canonical, machine, canonical, machine] {
            let memory = &memory;
            let start = &start;
            threads.spawn(move || {
                start.wait();
                for _ in 0..2000 {
                    match memory.get_blob(name) {
                        Ok(data) => assert_eq!(data.as_ref(), &bytes),
                        Err(error) => assert_eq!(error, StorageError::NotFound),
                    }
                }
            });
        }
        start.wait();
        for _ in 0..1000 {
            while !memory.evict(canonical) {
                std::thread::yield_now();
            }
            unsafe { memory.insert_canonical(canonical.object(), ImmutableBytes::new(&bytes)) };
        }
    });
    assert_eq!(retained.to_bytes().as_ref(), &bytes);
    assert_eq!(memory.blob_bytes(), bytes.len());
}
