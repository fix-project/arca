//! Repository archives, written and read with the system `tar`.

use crate::commands::CliError;
use fixrepository::Repository;
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn parent(repository: &Repository) -> PathBuf {
    Path::new(repository.path()).parent().unwrap().to_owned()
}

fn run(command: &mut Command, input: Option<&[u8]>) -> Result<(), CliError> {
    let failure = |error: std::io::Error| CliError::Backend(format!("tar: {error}"));
    let mut child = command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .spawn()
        .map_err(failure)?;
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input)
            .map_err(failure)?;
    }
    let status = child.wait().map_err(failure)?;
    if !status.success() {
        return Err(CliError::Backend(format!("tar exited with {status}")));
    }
    Ok(())
}

/// Writes `paths`, relative to the repository's parent, into a new archive.
pub fn export(
    repository: &Repository,
    mut paths: Vec<String>,
    archive: &Path,
) -> Result<(), CliError> {
    if archive.symlink_metadata().is_ok() {
        return Err(CliError::Backend(format!(
            "destination already exists: {}",
            archive.display()
        )));
    }
    paths.sort();
    paths.dedup();
    let headers = std::env::temp_dir().join(format!("fix-export-{}", std::process::id()));
    let directories = [".fix/data", ".fix/objects", ".fix/labels"];
    for directory in directories {
        std::fs::create_dir_all(headers.join(directory))
            .map_err(|error| CliError::Backend(error.to_string()))?;
    }
    let mut input = Vec::new();
    for path in &paths {
        input.extend_from_slice(path.as_bytes());
        input.push(0);
    }
    let result = run(
        Command::new("tar")
            .args(["--create", "--auto-compress", "--file"])
            .arg(archive)
            .args([
                "--no-recursion",
                "--mtime=@0",
                "--owner=0",
                "--group=0",
                "--numeric-owner",
            ])
            .arg("--directory")
            .arg(&headers)
            .args(directories)
            .arg("--directory")
            .arg(parent(repository))
            .args(["--null", "--files-from", "-"]),
        Some(&input),
    );
    let _ = std::fs::remove_dir_all(&headers);
    result
}

/// Extracts the `.fix` contents of an archive into the repository.
pub fn import(repository: &Repository, archive: &Path) -> Result<(), CliError> {
    run(
        Command::new("tar")
            .arg("--directory")
            .arg(parent(repository))
            .args(["--extract", "--file"])
            .arg(archive)
            .args(["--no-same-owner", "--no-same-permissions", ".fix"]),
        None,
    )
}
