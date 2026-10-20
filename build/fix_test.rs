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
        .env("FIX_TRUST", ".fix/trust")
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
    let mut seed = Command::new(locate(&env::var("FIX_TEST_SEED")?)?);
    seed.args(["seed", ".fix"])
        .arg(locate(&env::var("FIX_TEST_COMPILE")?)?);
    let mut labels = Vec::new();
    for (label, variable) in [
        ("parser", "FIX_TEST_PARSER"),
        ("identity", "FIX_TEST_IDENTITY"),
    ] {
        seed.arg(label).arg(locate(&env::var(variable)?)?);
        labels.push(label.to_owned());
    }
    for name in env::var("FIX_TEST_DATA")?.lines() {
        let path = locate(name)?;
        fs::copy(
            &path,
            PathBuf::from(path.file_name().ok_or("missing filename")?),
        )?;
        if fs::read(&path)?.starts_with(b"\x7fELF") {
            let label = path.file_stem().ok_or("missing program name")?;
            seed.arg(label).arg(&path);
            labels.push(label.to_str().ok_or("invalid program name")?.to_owned());
        }
    }
    assert!(seed.status()?.success(), "test certificate seeding failed");
    let mut bindings = String::new();
    for label in labels {
        let handle = call(&fix, &["resolve", &label], b"");
        bindings.push_str(&format!(
            "{label}\0{}\0",
            std::str::from_utf8(&handle)?.trim()
        ));
    }
    let bindings = call(
        &fix,
        &["create-tree", "--bindings", "-"],
        bindings.as_bytes(),
    );
    let handle = call(
        &fix,
        &[
            "run",
            &env::var("FIX_TEST_SCRIPT")?,
            std::str::from_utf8(&bindings)?.trim(),
        ],
        b"",
    );
    let value = call(
        &fix,
        &["read-blob", std::str::from_utf8(&handle)?.trim()],
        b"",
    );
    assert_eq!(hex::encode(value), env::var("FIX_TEST_EXPECTED")?);
    Ok(())
}
