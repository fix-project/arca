use fixhandle::{Blob, Handle, Tree};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn input(name: &str) -> PathBuf {
    PathBuf::from(env::var_os("TEST_SRCDIR").unwrap())
        .join(env::var_os("TEST_WORKSPACE").unwrap())
        .join(name)
}

fn run(program: &str, arguments: &[&str], directory: &Path, temporary: &Path) -> Output {
    let output = Command::new(input(program))
        .args(arguments)
        .current_dir(directory)
        .env("TMPDIR", temporary)
        .env("FIX_TRUST", directory.join("unrelated-trust"))
        .env("FIX_JOBS", "1")
        .output()
        .unwrap();
    assert!(
        fs::read_dir(temporary).unwrap().next().is_none(),
        "launcher did not remove its temporary repository"
    );
    output
}

fn result(output: Output, bytes: &[u8]) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected: Handle = Blob::literal(bytes).unwrap().into();
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        expected.to_string()
    );
}

#[test]
fn standalone_programs_compile_and_run_in_private_repositories() {
    let directory = PathBuf::from(env::var_os("TEST_TMPDIR").unwrap());
    let caller = directory.join("caller with spaces");
    let temporary = directory.join("temporary");
    fs::create_dir(&caller).unwrap();
    fs::create_dir(&temporary).unwrap();
    fs::write(caller.join(".fix"), b"unrelated repository").unwrap();
    fs::write(
        caller.join("unrelated-trust"),
        b"invalid trust configuration",
    )
    .unwrap();
    for (program, arguments, bytes) in [
        (env!("WAT_BLOB"), Vec::new(), b"hello, world".to_vec()),
        (env!("C_BLOB"), Vec::new(), b"hello, world".to_vec()),
        (env!("RUST_BLOB"), Vec::new(), b"hello, world".to_vec()),
        (
            env!("WAT_ADD"),
            vec!["20u64", "22u64"],
            42u64.to_le_bytes().to_vec(),
        ),
        (
            env!("C_ADD"),
            vec!["20u64", "22u64"],
            42u64.to_le_bytes().to_vec(),
        ),
        (
            env!("RUST_ADD"),
            vec!["20u64", "22u64"],
            42u64.to_le_bytes().to_vec(),
        ),
    ] {
        result(run(program, &arguments, &caller, &temporary), &bytes);
    }
    fs::write(caller.join("input 雪.bin"), 40u64.to_le_bytes()).unwrap();
    result(
        run(
            env!("RUST_ADD"),
            &["@file(\"input 雪.bin\")", "2u64"],
            &caller,
            &temporary,
        ),
        &42u64.to_le_bytes(),
    );
    result(
        run(env!("FIB"), &["8u64"], &caller, &temporary),
        &34u64.to_le_bytes(),
    );
    let trap = Tree::canonical(&[
        Blob::literal(b"Wasm.trap").unwrap().into(),
        Blob::literal(&1u32.to_le_bytes()).unwrap().into(),
    ])
    .unwrap();
    let trapped = run(env!("ADD_BLOB"), &[], &caller, &temporary);
    assert!(
        trapped.status.success(),
        "{}",
        String::from_utf8_lossy(&trapped.stderr)
    );
    assert_eq!(
        String::from_utf8(trapped.stdout).unwrap().trim(),
        Handle::from(trap).to_string()
    );
    let failure = run(env!("RUST_ADD"), &["("], &caller, &temporary);
    assert_eq!(failure.status.code(), Some(2));
    assert!(!failure.stderr.is_empty());
    assert_eq!(
        fs::read(caller.join(".fix")).unwrap(),
        b"unrelated repository"
    );
    assert_eq!(
        fs::read(caller.join("unrelated-trust")).unwrap(),
        b"invalid trust configuration"
    );
}
