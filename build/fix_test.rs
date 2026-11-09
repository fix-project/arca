//! Imports the bootstrap seed, adds test programs as Runnables, then runs a Fix script.

use runfiles::Runfiles;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn call(fix: &Path, args: &[&str]) -> String {
    let output = Command::new(fix)
        .args(args)
        .env("FIX_JOBS", env::var("FIX_TEST_JOBS").unwrap())
        .env("FIX_TRUST", ".fix/trust")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fix {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runfiles = Runfiles::create()?;
    let locate = |name: &str| runfiles.rlocation(name).ok_or("missing test input");
    let fix = locate(&env::var("FIX_TEST_CLI")?)?;
    env::set_current_dir(env::var_os("TEST_TMPDIR").ok_or("missing test directory")?)?;
    call(&fix, &["init"]);
    for name in env::var("FIX_TEST_ARCHIVES")?.lines() {
        call(
            &fix,
            &["import", locate(name)?.to_str().ok_or("non-UTF-8 path")?],
        );
    }
    let mut seed = Command::new(locate(&env::var("FIX_TEST_SEED")?)?);
    seed.args(["add", "."]);
    let mut labels = Vec::new();
    for name in env::var("FIX_TEST_DATA")?.lines() {
        let path = locate(name)?;
        fs::copy(
            &path,
            PathBuf::from(path.file_name().ok_or("missing filename")?),
        )?;
        if fs::read(&path)?.starts_with(b"\x7fELF") {
            let label = path
                .file_stem()
                .ok_or("missing program name")?
                .to_str()
                .ok_or("invalid program name")?;
            seed.arg(format!("{label}={}", path.display()));
            labels.push(label.to_owned());
        }
    }
    assert!(seed.status()?.success(), "test seeding failed");
    let mut arguments = vec!["run".to_owned(), env::var("FIX_TEST_SCRIPT")?];
    for label in labels {
        arguments.extend(["-b".into(), format!("{label}=@label(\"{label}\")")]);
    }
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let handle = call(&fix, &arguments);
    let value = Command::new(&fix).args(["read-blob", &handle]).output()?;
    assert!(value.status.success(), "fix read-blob failed");
    assert_eq!(hex::encode(value.stdout), env::var("FIX_TEST_EXPECTED")?);
    Ok(())
}
