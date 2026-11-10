use fix::{EncodeType, TypeError};
use fixcli::shell::{Blob, Encode, Object, Reference, Thunk, Tree, Value};
use fixcli::{Options, Shell as Client};

fn equal<T: Eq>(left: &T, right: &T) -> bool {
    left == right
}

#[test]
fn typed_values_use_the_linux_shell_and_preserve_ownership() {
    let directory =
        std::path::PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("typed-values");
    fixrepository::Repository::init(directory.join(".fix").to_str().unwrap()).unwrap();
    let mut shell = Client::open(&directory, Options::default()).unwrap();

    let empty_blob = Blob::default();
    assert!(empty_blob.is_empty());
    assert_eq!(empty_blob, Blob::from(&[][..]));
    let empty_tree = Tree::default();
    assert!(empty_tree.is_empty());
    assert_eq!(empty_tree, Tree::new(&[]));
    assert_ne!(Value::from(empty_tree), Value::default());
    assert_eq!(Object::default().data_type(), fix::DataType::Blob);

    let contents: Vec<u8> = (0..128).collect();
    let blob = Blob::new(&contents);
    assert!(equal(&blob, &Blob::new(&contents)));
    assert_eq!(blob, Blob::from(&blob));
    assert_ne!(blob, Blob::from(&contents[..127]));
    assert_ne!(blob, Blob::new(&[255; 128]));
    let number = Blob::from(42u64);
    let tree = Tree::new(&[blob.clone().into(), number.into()]);
    assert_eq!(tree.len(), 2);
    assert!(!tree.is_tag());
    assert_eq!(tree, Tree::from(&tree));
    assert_ne!(tree, Tree::default());
    let opaque = Tree::from([Tree::default().application().into()]);
    assert!(opaque.equals(&opaque).is_err());
    assert_ne!(opaque, opaque.clone());
    assert_ne!(opaque, opaque);
    let opaque_value: Value = opaque.into();
    assert_ne!(opaque_value, opaque_value);

    let object: Object = tree.clone().into();
    let reference = object.reference();
    assert_eq!(reference.len(), 2);
    let identification = reference.identification();
    let request = identification.strict();
    assert_eq!(request.encode_type(), EncodeType::Strict);
    let request: Value = request.into();
    let request = Encode::try_from(request).unwrap();
    assert_eq!(request.encode_type(), EncodeType::Strict);
    assert!(matches!(
        Thunk::try_from(request.into_value()),
        Err(TypeError::WrongType)
    ));
    assert!(matches!(
        Blob::try_from(reference.clone().into_value()),
        Err(TypeError::WrongType)
    ));
    let _: Reference = Reference::try_from(reference.into_value()).unwrap();

    assert!(tree.get(2).unwrap().is_none());
    let mut iterator = tree.iter();
    assert_eq!(iterator.len(), 2);
    let last = Blob::try_from(iterator.next_back().unwrap().unwrap()).unwrap();
    let mut number = [0; 8];
    last.read_exact(0, &mut number).unwrap();
    assert_eq!(u64::from_le_bytes(number), 42);
    assert_eq!(iterator.len(), 1);
    let mut copied = iterator.clone();
    assert_eq!(copied.len(), 1);
    assert_eq!(
        copied.next().unwrap().unwrap(),
        tree.get(0).unwrap().unwrap()
    );
    assert_eq!(iterator.len(), 1);
    assert!(iterator.next().unwrap().is_ok());
    assert!(iterator.next().is_none());
    assert!(iterator.next_back().is_none());
    let children: Vec<_> = tree.clone().into_iter().collect::<Result<_, _>>().unwrap();
    assert_eq!(children.len(), 2);
    let rebuilt: Tree = tree.iter().collect::<Result<_, _>>().unwrap();
    assert!(rebuilt.equals(&tree).unwrap());
    let from_values: Tree = children.into_iter().collect();
    assert!(from_values.equals(&tree).unwrap());
    let mut borrowed = 0;
    for child in &tree {
        assert!(child.is_ok());
        borrowed += 1;
    }
    assert_eq!(borrowed, 2);
    let child = tree.get(0).unwrap().unwrap();
    assert!(child.equals(blob.as_value()).unwrap());
    let child = Blob::try_from(child).unwrap();
    assert_eq!(child.len(), 128);
    drop(tree);
    drop(blob);
    let mut bytes = [0; 17];
    child.read_exact(31, &mut bytes).unwrap();
    assert_eq!(bytes, contents[31..48]);
    assert!(child.read_exact(127, &mut bytes).is_err());
    bytes.fill(0);
    assert_eq!(child.read(127, &mut bytes).unwrap(), 1);
    assert_eq!(bytes[0], 127);
    assert_eq!(child.read(128, &mut bytes).unwrap(), 0);
    let saved = shell.save(child.as_value()).unwrap();
    shell.close().unwrap();

    let mut shell = Client::open(&directory, Options::default()).unwrap();
    let restored = shell.lookup(&saved).unwrap();
    let restored = Blob::try_from(restored).unwrap();
    assert_eq!(restored, Blob::new(&contents));
    let mut bytes = [0; 128];
    restored.read_exact(0, &mut bytes).unwrap();
    assert_eq!(bytes.as_slice(), contents);
}
