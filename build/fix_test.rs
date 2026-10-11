use runfiles::Runfiles;
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn call(fix: &Path, args: &[&str], input: &[u8]) -> Vec<u8> {
    let mut child = Command::new(fix)
        .args(args)
        .env_remove("BUILD_WORKSPACE_DIRECTORY")
        .env("FIX_JOBS", env::var("FIX_TEST_JOBS").unwrap())
        .env(
            "PATH",
            env::join_paths(
                std::iter::once(fix.parent().unwrap().join("commands/bin"))
                    .chain(env::split_paths(&env::var_os("PATH").unwrap_or_default())),
            )
            .unwrap(),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "fix {args:?} failed");
    output.stdout
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runfiles = Runfiles::create()?;
    let locate = |name: &str| runfiles.rlocation(name).ok_or("missing test input");
    let fix = locate(&env::var("FIX_TEST_CLI")?)?;
    env::set_current_dir(env::var_os("TEST_TMPDIR").ok_or("missing test directory")?)?;
    call(&fix, &["init"], b"");
    for (label, variable) in [
        ("parser", "FIX_TEST_PARSER"),
        ("identity", "FIX_TEST_IDENTITY"),
    ] {
        call(
            &fix,
            &["create-blob", "-", "-l", label],
            &fs::read(locate(&env::var(variable)?)?)?,
        );
    }
    for name in env::var("FIX_TEST_DATA")?.lines() {
        let path = locate(name)?;
        fs::copy(
            &path,
            PathBuf::from(path.file_name().ok_or("missing filename")?),
        )?;
    }
    let handle = call(&fix, &["run", &env::var("FIX_TEST_SCRIPT")?], b"");
    let value = call(
        &fix,
        &["read-blob", std::str::from_utf8(&handle)?.trim()],
        b"",
    );
    assert_eq!(hex::encode(value), env::var("FIX_TEST_EXPECTED")?);
    Ok(())
}
