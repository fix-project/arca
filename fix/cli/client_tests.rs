use fixapi::{DataType, Fix, ValueType};
use fixcli::{Client, Options};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn input(name: &str) -> PathBuf {
    PathBuf::from(std::env::var("TEST_SRCDIR").unwrap())
        .join(std::env::var("TEST_WORKSPACE").unwrap())
        .join(name)
}
fn repository(name: &str) -> PathBuf {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join(name);
    std::fs::create_dir(&directory).unwrap();
    let output = Command::new(input(env!("BACKEND")))
        .args(["init"])
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(output.status.success());
    std::fs::create_dir(directory.join("toolchain")).unwrap();
    let output = Command::new(input(env!("SEED")))
        .args(["create", "."])
        .arg(input(env!("COMPILER")))
        .arg(input(env!("ADD")))
        .arg(input(env!("ADD")))
        .arg("toolchain")
        .arg(format!("scheduler={}", input(env!("SCHEDULER")).display()))
        .arg(format!("collector={}", input(env!("COLLECTOR")).display()))
        .arg(format!("add={}", input(env!("ADD")).display()))
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    directory
}
fn client(directory: &Path) -> Client {
    Client::open(
        directory,
        Options {
            trust: Some(directory.join(".fix/trust")),
            memory_mib: 128,
            ..Options::default()
        },
    )
    .unwrap()
}

#[test]
fn local_values_can_be_read_without_an_evaluator_and_saved() {
    let directory = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap()).join("local-values");
    fixrepository::Repository::init(directory.join(".fix").to_str().unwrap()).unwrap();
    let mut client = Client::open(&directory, Options::default()).unwrap();
    let contents: Vec<u8> = (0..128).collect();
    let blob = client.blob(&contents).unwrap();
    let number = client.blob(&42u64.to_le_bytes()).unwrap();
    let tree = client.tree(&[blob.clone(), number]).unwrap();
    assert_eq!(client.value_type(&tree).unwrap(), ValueType::Object);
    assert_eq!(client.data_type(&tree).unwrap(), DataType::Tree);
    assert_eq!(client.len(&tree).unwrap(), 2);
    assert!(!client.is_tag(&tree).unwrap());
    assert!(client.is_comparable(&tree).unwrap());
    let reference = client.reference(&tree).unwrap();
    assert_eq!(client.value_type(&reference).unwrap(), ValueType::Ref);
    let child = client.get(&tree, 0).unwrap();
    assert!(client.equals(&child, &blob).unwrap());
    assert_eq!(client.data_type(&child).unwrap(), DataType::Blob);
    assert_eq!(client.len(&child).unwrap(), contents.len() as u64);
    let mut slice = [0; 17];
    client.read(&child, 31, &mut slice).unwrap();
    assert_eq!(slice, contents[31..48]);
    let name = client.save(&tree).unwrap();
    drop(tree);
    drop(blob);
    client.read(&child, 31, &mut slice).unwrap();
    assert_eq!(slice, contents[31..48]);
    client.close().unwrap();

    let mut client = Client::open(&directory, Options::default()).unwrap();
    let restored = client.lookup(&name).unwrap();
    let child = client.get(&restored, 0).unwrap();
    let mut result = vec![0; contents.len()];
    client.read(&child, 0, &mut result).unwrap();
    assert_eq!(result, contents);
}

#[test]
fn persistent_session_values_evaluation_and_storage() {
    let directory = repository("roundtrip");
    let mut session = client(&directory);
    session.reuse_vm();
    let left = session.blob(&20u64.to_le_bytes()).unwrap();
    let right = session.blob(&22u64.to_le_bytes()).unwrap();
    let add = session.lookup("add").unwrap();
    let metadata = session.blob(b"").unwrap();
    let combination = session.tree(&[metadata, add, left, right]).unwrap();
    let application = session.application(&combination).unwrap();
    assert_eq!(session.value_type(&application).unwrap(), ValueType::Thunk);
    let request = session.strict(&application).unwrap();
    let result = session.eval(&request).unwrap();
    let invalid = session.blob(b"not a combination").unwrap();
    assert!(session.call(&invalid).is_err());
    let trust_path = directory.join(".fix/trust");
    let trust = std::fs::read(&trust_path).unwrap();
    std::fs::write(&trust_path, b"invalid trust configuration").unwrap();
    assert!(session.eval(&request).is_err());
    std::fs::write(&trust_path, trust).unwrap();
    let repeated = session.eval(&request).unwrap();
    assert_eq!(
        session.save(&repeated).unwrap(),
        session.save(&result).unwrap()
    );
    let mut bytes = [0; 8];
    session.read(&result, 0, &mut bytes).unwrap();
    assert_eq!(u64::from_le_bytes(bytes), 42);
    session.label("answer", &result).unwrap();
    let name = session.save(&result).unwrap();
    session.close().unwrap();
    let mut session = client(&directory);
    let result = session.lookup("answer").unwrap();
    assert_eq!(session.save(&result).unwrap(), name);
    session.close().unwrap();
}

#[test]
fn a_failed_invocation_does_not_terminate_the_host() {
    let directory = repository("failed-invocation");
    let mut session = client(&directory);
    let add = session.lookup("add").unwrap();
    let metadata = session.blob(b"").unwrap();
    let combination = session.tree(&[metadata, add]).unwrap();
    assert!(session.call(&combination).is_err());
    for number in [20u64, 21] {
        let metadata = session.blob(b"").unwrap();
        let add = session.lookup("add").unwrap();
        let argument = session.blob(&number.to_le_bytes()).unwrap();
        let combination = session
            .tree(&[metadata, add, argument.clone(), argument])
            .unwrap();
        let result = session.call(&combination).unwrap();
        let mut bytes = [0; 8];
        session.read(&result, 0, &mut bytes).unwrap();
        assert_eq!(u64::from_le_bytes(bytes), number * 2);
    }
    session.close().unwrap();
}

#[test]
fn coupon_persistence_modes() {
    fn claims(directory: &Path) -> std::collections::BTreeSet<String> {
        fixrepository::Repository::open(directory.join(".fix").to_str().unwrap())
            .unwrap()
            .claims(&[])
            .unwrap()
            .iter()
            .map(|tree| tree.as_handle().to_string())
            .collect()
    }
    for mode in [
        fixcli::Memoization::Auto,
        fixcli::Memoization::Always,
        fixcli::Memoization::Never,
    ] {
        let directory = repository(&format!("memoize-{mode}"));
        let before = claims(&directory);
        let mut session = Client::open(
            &directory,
            Options {
                trust: Some(directory.join(".fix/trust")),
                memory_mib: 128,
                memoize: mode,
                ..Options::default()
            },
        )
        .unwrap();
        let left = session.blob(&20u64.to_le_bytes()).unwrap();
        let right = session.blob(&22u64.to_le_bytes()).unwrap();
        let add = session.lookup("add").unwrap();
        let metadata = session.blob(b"").unwrap();
        let combination = session.tree(&[metadata, add, left, right]).unwrap();
        let application = session.application(&combination).unwrap();
        let request = session.strict(&application).unwrap();
        let result = session.eval(&request).unwrap();
        let mut bytes = [0; 8];
        session.read(&result, 0, &mut bytes).unwrap();
        assert_eq!(u64::from_le_bytes(bytes), 42);
        let after = claims(&directory);
        if mode == fixcli::Memoization::Never {
            assert_eq!(after, before);
        } else {
            assert!(after.difference(&before).next().is_some());
        }
        let again = session.eval(&request).unwrap();
        session.read(&again, 0, &mut bytes).unwrap();
        assert_eq!(u64::from_le_bytes(bytes), 42);
        session.close().unwrap();
    }
}

#[test]
fn persistent_session_reports_invocation_errors() {
    let directory = repository("failed-session");
    let mut session = client(&directory);
    session.reuse_vm();
    let add = session.lookup("add").unwrap();
    let metadata = session.blob(b"").unwrap();
    let combination = session.tree(&[metadata, add]).unwrap();
    assert!(session.call(&combination).is_err());
    session.close().unwrap();
}
