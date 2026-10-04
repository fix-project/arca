#![no_main]
#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(crate::testing::test_runner)]
#![reexport_test_harness_main = "test_main"]

mod parallel_evaluator;
mod scheduler;
use kernel::host::fs;
use kernel::host::os;
use kernel::prelude::*;

use fix::arca::FixOnArca;
use fix::*;

pub const PARSER: &[u8] = include_bytes!(env!("FIX_PARSER"));

#[cfg(test)]
mod testing;

#[cfg(test)]
#[kmain]
fn tests() {
    test_main();
}

#[cfg_attr(not(test), kmain)]
#[cfg_attr(test, allow(dead_code))]
fn main() {
    let argv = os::argv();

    // Subcommand dispatch: `fix init` | `fix eval <file>`.
    match argv.get(1).map(String::as_str) {
        Some("init") => init(),
        Some("eval") => {
            let path = argv.get(2).expect("fix eval: expected a command file");
            eval_file(path)
        }
        // test to run the parallel evaluator
        Some("parallel_eval") => {
            let path = argv.get(2).expect("fix eval: expected a command file");
            eval_file_parallel(path);
        }
        Some(other) => panic!("fix: unknown command '{other}' (expected: init | eval <file> )"),
        None => panic!("fix: expected a command (init | eval <file> "),
    }

    kernel::shutdown();
}

/// `fix init`: create the on-disk `.fix` store with its `objects/` and
/// `labels/` subdirs. `mkdir` maps to host `create_dir_all`, so re-running on an
/// existing store is harmless (matches git's "reinitialized existing repository").
fn init() {
    for dir in [".fix/objects", ".fix/labels"] {
        if let Err(e) = fs::mkdir(dir) {
            println!("fix init: failed to create {dir}: {e:?}");
            kernel::exit(1);
        }
    }
    println!("initialized empty fix store in .fix");
}

// Jennifer: tons of redundancy but I just didn't want to change original code,
// in case errors showed up
// the main change is just calling the parallel evaluator and how its passed in
fn eval_file_parallel(path: &str) {
    let file = preprocessor::read_file(path).unwrap();

    let evaluator = parallel_evaluator::Evaluator::new(FixOnArca::default());
    let result = eval_parallel_program(core::str::from_utf8(&file).unwrap(), &evaluator);

    println!("handle:    {result}");
    println!("Current handle is: {:?}", result);
    if let Ok(blob) = fixhandle::Blob::try_from(result) {
        let contents = evaluator.storage().get_blob(blob).unwrap();
        println!("result is a Blob: {contents:?}");
        if contents.len() == 8 {
            let bytes: [u8; 8] = (*contents).try_into().unwrap();
            println!("\tas a u64: {}", u64::from_le_bytes(bytes));
        }
    }
}

fn eval_parallel_program(
    source: &str,
    evaluator: &Arc<parallel_evaluator::Evaluator<FixOnArca>>,
) -> Handle {
    let processed = Preprocessor::new(source).preprocess().unwrap();
    let parser: Handle = evaluator.storage().add_blob(PARSER).into();
    let source = evaluator.storage().add_blob(processed.as_bytes());
    let environment = stdlib::build_environment(evaluator.storage());
    let combination = evaluator
        .storage()
        .add_tree(&[parser, source.into(), environment]);

    evaluator.eval(combination.into_ref().application().strict().into())
}

// `fix eval <file>`: read command file and print result.
fn eval_file(path: &str) {
    let file = preprocessor::read_file(path).unwrap();
    let evaluator = Evaluator::new(FixOnArca::default());
    let result = eval_program(core::str::from_utf8(&file).unwrap(), &evaluator);

    println!("handle:    {result}");
    println!("Current handle is: {:?}", result);
    if let Ok(blob) = fixhandle::Blob::try_from(result) {
        let contents = evaluator.storage().get_blob(blob).unwrap();
        println!("result is a Blob: {contents:?}");
        if contents.len() == 8 {
            let bytes: [u8; 8] = (*contents).try_into().unwrap();
            println!("\tas a u64: {}", u64::from_le_bytes(bytes));
        }
    }
}

// parse, interpret, and evaluate source text.
fn eval_program(source: &str, evaluator: &Evaluator<FixOnArca>) -> Handle {
    let processed = Preprocessor::new(source).preprocess().unwrap();
    let parser: Handle = evaluator.storage().add_blob(PARSER).into();
    let source = evaluator.storage().add_blob(processed.as_bytes());
    let environment = stdlib::build_environment(evaluator.storage());
    let combination = evaluator
        .storage()
        .add_tree(&[parser, source.into(), environment]);

    evaluator.eval(combination.into_ref().application().strict().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fix::storage::CanonicalHandle;
    use fixhandle::{Blob, Tree};

    #[test_case]
    fn canonical_blob_round_trip() {
        let storage = storage::memory::MemoryStorage::default();
        let bytes = [42u8; 64];
        let local = storage.add_blob(&bytes);
        let canonical = storage.canonicalize_blob(local).unwrap();
        let handle = Handle::from(canonical);
        let blob = Blob::try_from(handle).unwrap();
        assert_eq!(storage.get_blob(blob).unwrap().as_ref(), &bytes);

        let duplicate = storage.add_blob(&bytes);
        assert_ne!(
            local.as_handle().as_bytes(),
            duplicate.as_handle().as_bytes()
        );
        assert_eq!(storage.canonicalize_blob(duplicate), Ok(canonical));

        let decoded =
            Blob::try_from(unsafe { Handle::parse(*handle.as_bytes()) }.unwrap()).unwrap();
        assert_eq!(CanonicalHandle::try_from(decoded).unwrap(), canonical);
        assert_eq!(storage.get_blob(decoded).unwrap().as_ref(), &bytes);
    }

    #[test_case]
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

    #[test_case]
    fn equality_of_aliases_and_computed_values() {
        use fix::storage::{StorageError, memory::MemoryStorage};
        let storage = MemoryStorage::default();
        let a = storage.add_blob(&[7; 64]);
        let b = storage.add_blob(&[7; 64]);
        let c = storage.add_blob(&[8; 64]);
        assert!(a.as_handle().is_eq());
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
        assert!(left.as_handle().is_eq());
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
            assert!(!value.is_eq());
            assert_eq!(storage.equals(value, value), Err(StorageError::NotEq));
            let tree = storage.add_tree(&[value]);
            let canonical: Handle = storage.canonicalize_tree(tree).unwrap().into();
            assert!(canonical.is_canonical());
            assert!(!canonical.is_eq());
            assert_eq!(
                storage.equals(canonical, canonical),
                Err(StorageError::NotEq)
            );
            assert!(
                !Tree::try_from(canonical)
                    .unwrap()
                    .into_ref()
                    .as_handle()
                    .is_eq()
            );
        }
        let empty = storage.add_tree(&[]);
        assert!(empty.as_handle().is_eq());
        assert_eq!(storage.equals(empty.into(), empty.into()), Ok(true));
        let mut forged = *left.as_handle().as_bytes();
        forged[30] ^= 4;
        assert!(matches!(
            storage.get_tree(Tree::try_from(unsafe { Handle::parse(forged) }.unwrap()).unwrap()),
            Err(StorageError::InvalidEq)
        ));
    }

    #[test_case]
    fn equality_after_encoded_substitution() {
        let evaluator = Evaluator::new(FixOnArca::default());
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
        assert!(!first.as_handle().is_eq() && !second.as_handle().is_eq());
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
        assert!(first.is_eq() && second.is_eq());
        assert_eq!(storage.equals(first, second), Ok(true));
    }

    fn eval_value(source: &str, evaluator: &Evaluator<FixOnArca>) -> Vec<u8> {
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

    #[test_case]
    fn test_memory_storage_handle_boundaries() {
        use fix::storage::{StorageError, memory::MemoryStorage};

        let storage = MemoryStorage::default();
        let other = MemoryStorage::default();
        let contents = [0x5a; 31];
        let blob = storage.add_blob(&contents);
        let empty = storage.add_blob(&[]);
        let children = [empty.into(), blob.into()];
        let tree = storage.add_tree(&children);

        assert_eq!(storage.get_blob(blob).unwrap().as_ref(), &contents);
        assert!(storage.get_blob(empty).unwrap().is_empty());
        assert!(other.get_blob(empty).unwrap().is_empty());
        assert!(matches!(
            other.get_blob(blob),
            Err(StorageError::WrongStorage)
        ));
        assert!(matches!(
            other.get_tree(tree),
            Err(StorageError::WrongStorage)
        ));

        let Name::Machine { index, storage_id } = blob.as_handle().name() else {
            panic!("expected machine blob");
        };
        let wrong_length =
            unsafe { fixhandle::Blob::named(Name::Machine { index, storage_id }, 32, false) }
                .unwrap();
        assert!(matches!(
            storage.get_blob(wrong_length),
            Err(StorageError::LengthMismatch)
        ));
        let missing = unsafe {
            fixhandle::Blob::named(
                Name::Machine {
                    index: index + 1,
                    storage_id,
                },
                contents.len() as u64,
                false,
            )
        }
        .unwrap();
        assert!(matches!(
            storage.get_blob(missing),
            Err(StorageError::NotFound)
        ));
        let local = unsafe {
            fixhandle::Blob::named(
                Name::Local {
                    address: contents.as_ptr() as u64,
                },
                contents.len() as u64,
                true,
            )
        }
        .unwrap();
        assert!(matches!(
            storage.get_blob(local),
            Err(StorageError::UnsupportedName)
        ));

        let wrong_tree_length =
            unsafe { Tree::named(tree.as_handle().name(), HANDLE_SIZE as u64, false) }.unwrap();
        assert!(matches!(
            storage.get_tree(wrong_tree_length),
            Err(StorageError::LengthMismatch)
        ));
        let local_tree = unsafe {
            Tree::named(
                Name::Local {
                    address: children.as_ptr() as u64,
                },
                core::mem::size_of_val(&children) as u64,
                false,
            )
        }
        .unwrap();
        assert!(matches!(
            storage.get_tree(local_tree),
            Err(StorageError::UnsupportedName)
        ));

        assert_eq!(tree.len(), children.len());
        assert_eq!(tree.byte_len(), (HANDLE_SIZE * children.len()) as u64);
        assert_eq!(
            &tree.as_handle().as_bytes()[24..30],
            &((HANDLE_SIZE * children.len()) as u64).to_le_bytes()[..6]
        );
        let retrieved = storage.get_tree(tree).unwrap();
        assert_eq!(
            retrieved.as_ptr() as usize % core::mem::align_of::<Handle>(),
            0
        );
        assert_eq!(retrieved.len(), children.len());
        for (actual, expected) in retrieved.iter().zip(children.iter()) {
            assert_eq!(actual.as_bytes(), expected.as_bytes());
        }
    }

    #[test_case]
    fn test_shared_storage_pages() {
        use fix::storage::memory::MemoryStorage;
        use kernel::types::Entry;
        let storage = MemoryStorage::default();
        let bytes = [0x5a; 8193];
        let blob = storage.add_blob(&bytes);
        let first = storage.get_blob_backing(blob).unwrap();
        let second = storage.get_blob_backing(blob).unwrap();
        let Entry::ROPage(mut a) = first.tables()[0].get(0).unwrap() else {
            panic!()
        };
        let Entry::ROPage(b) = second.tables()[0].get(0).unwrap() else {
            panic!()
        };
        assert_eq!(a.with_ref(|p| p.as_ptr()), b.with_ref(|p| p.as_ptr()));
        a.write(0, &[1]);
        assert_ne!(a.with_ref(|p| p.as_ptr()), b.with_ref(|p| p.as_ptr()));
        assert_eq!(storage.get_blob(blob).unwrap().as_ref(), &bytes);
        let tree = storage.add_tree(&[blob.into(), blob.into()]);
        let first = storage.get_tree_backing(tree).unwrap();
        let second = storage.get_tree_backing(tree).unwrap();
        let Entry::ROPage(a) = first.tables()[0].get(0).unwrap() else {
            panic!()
        };
        let Entry::ROPage(b) = second.tables()[0].get(0).unwrap() else {
            panic!()
        };
        assert_eq!(a.with_ref(|p| p.as_ptr()), b.with_ref(|p| p.as_ptr()));
        assert_eq!(first.len(), 2 * HANDLE_SIZE);
        assert!(a.with_ref(|p| p[2 * HANDLE_SIZE..].iter().all(|b| *b == 0)));
    }

    #[test_case]
    fn test_nested_shared_backing() {
        use fix::storage::ImmutableBytes;
        use kernel::types::Entry;
        let mut bytes = Vec::new();
        bytes.resize((1 << 21) + 1, 0x29);
        let backing = ImmutableBytes::new(&bytes);
        let a = &backing.tables()[0];
        let b = a.clone();
        let (Entry::ROTable(a) | Entry::RWTable(a)) = a.get(1).unwrap() else {
            panic!()
        };
        let (Entry::ROTable(b) | Entry::RWTable(b)) = b.get(1).unwrap() else {
            panic!()
        };
        match (a.inner(), b.inner()) {
            (
                kernel::types::internal::Table::Table2MB(kernel::prelude::CowPage::Shared(a)),
                kernel::types::internal::Table::Table2MB(kernel::prelude::CowPage::Shared(b)),
            ) => {
                assert!(core::ptr::eq(&**a, &**b));
            }
            _ => panic!("expected shared child Tables"),
        }
        let Entry::ROPage(a) = a.get(0).unwrap() else {
            panic!()
        };
        let Entry::ROPage(b) = b.get(0).unwrap() else {
            panic!()
        };
        assert_eq!(
            a.with_ref(|page| page.as_ptr()),
            b.with_ref(|page| page.as_ptr())
        );
        assert_eq!(&*backing.to_bytes(), bytes.as_slice());
    }

    #[test_case]
    fn test_sparse_chunk_container() {
        use kernel::types::{Entry, Page, Table, Value};
        let mut first = Table::new(1 << 30);
        let mut last = Page::new(4096);
        last.write(4095, &[0x29]);
        assert!(first.map((1 << 30) - 4096, Entry::ROPage(last)).is_ok());
        let mut second = Table::new(1 << 30);
        let mut start = Page::new(4096);
        start.write(0, &[0x5a]);
        assert!(second.map(0, Entry::ROPage(start)).is_ok());
        let Value::Tuple(chunks) = fix::storage::backing_value(&[first, second]) else {
            panic!()
        };
        assert_eq!(chunks.len(), 2);
        let Value::Table(first) = chunks.get(0) else {
            panic!()
        };
        let Value::Table(second) = chunks.get(1) else {
            panic!()
        };
        assert_eq!(first.len(), 1 << 30);
        assert_eq!(second.len(), 1 << 30);
        let Entry::RWTable(first) = first.get(511).unwrap() else {
            panic!()
        };
        let Entry::ROPage(last) = first.get(511).unwrap() else {
            panic!()
        };
        let Entry::RWTable(second) = second.get(0).unwrap() else {
            panic!()
        };
        let Entry::ROPage(start) = second.get(0).unwrap() else {
            panic!()
        };
        assert_eq!(last.with_ref(|page| page[4095]), 0x29);
        assert_eq!(start.with_ref(|page| page[0]), 0x5a);
    }

    #[test_case]
    fn test_number() {
        let evaluator = Evaluator::new(FixOnArca::default());

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
                eval_value("*[(1u8 2u8 3u8 4u8) 2u8]", &evaluator),
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
                eval_value("*#($identity 2u8)", &evaluator),
                2u64.to_le_bytes()
            );
        }
    }
}
