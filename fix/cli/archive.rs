//! Repository archives, written and read with the system `tar`.

use crate::commands::CliError;
use fixrepository::Repository;
use std::{
    io::Write,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> std::io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "fix-export-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(error) => return Err(error),
            }
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

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
    let temporary = Directory::new().map_err(|error| CliError::Backend(error.to_string()))?;
    let headers = &temporary.0;
    let staging = Repository::init(headers.join(".fix").to_str().unwrap())?.unsigned();
    let directories = [".fix/data", ".fix/objects", ".fix/labels"];
    for directory in directories {
        std::fs::create_dir_all(headers.join(directory))
            .map_err(|error| CliError::Backend(error.to_string()))?;
    }
    let mut staged = Vec::new();
    let policy = repository.policy()?;
    if !policy.unsigned && !policy.identities.is_empty() {
        let mut originals = Vec::new();
        for path in paths {
            let name = path.strip_prefix(".fix/objects/");
            let mut bytes = [0; 32];
            if name.is_some_and(|name| hex::decode_to_slice(name, &mut bytes).is_ok())
                && bytes[31] & 0x40 != 0
            {
                let tag = repository.lookup(&bytes)?;
                let object = fixhandle::Object::try_from(tag)
                    .map_err(fixrepository::RepositoryError::from)
                    .or_else(|_| {
                        fixhandle::Ref::try_from(tag)
                            .map(|reference| reference.object_descriptor())
                            .map_err(fixrepository::RepositoryError::from)
                    })?;
                staging.publish_reference(object.into_ref())?;
                staging.retain_certificates(&bytes, &repository.export_certificates(tag)?)?;
                staged.extend(staging.object_paths(object, false)?);
            } else {
                originals.push(path);
            }
        }
        paths = originals;
    }
    staged.sort();
    staged.dedup();
    paths.retain(|path| staged.binary_search(path).is_err());
    let mut input = Vec::new();
    for path in &paths {
        input.extend_from_slice(path.as_bytes());
        input.push(0);
    }
    run(
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
            .arg(headers)
            .args(directories)
            .args(staged)
            .arg("--directory")
            .arg(parent(repository))
            .args(["--null", "--files-from", "-"]),
        Some(&input),
    )
}

/// Extracts the `.fix` contents of an archive into the repository.
pub fn import(repository: &Repository, archive: &Path, sign: bool) -> Result<(), CliError> {
    let listing = Command::new("tar")
        .args(["--list", "--quoting-style=escape", "--file"])
        .arg(archive)
        .output()
        .map_err(|error| CliError::Backend(format!("tar: {error}")))?;
    if !listing.status.success() {
        return Err(CliError::Backend(
            String::from_utf8_lossy(&listing.stderr).into_owned(),
        ));
    }
    let mut tags = Vec::new();
    for path in String::from_utf8_lossy(&listing.stdout).lines() {
        let Some(name) = path.trim_start_matches("./").strip_prefix(".fix/objects/") else {
            continue;
        };
        let mut bytes = [0; 32];
        if hex::decode_to_slice(name, &mut bytes).is_ok() && bytes[31] & 0x40 != 0 {
            tags.push(bytes);
        }
    }
    run(
        Command::new("tar")
            .arg("--directory")
            .arg(parent(repository))
            .args(["--extract", "--file"])
            .arg(archive)
            .args([
                "--no-same-owner",
                "--no-same-permissions",
                "--exclude=.fix/config",
                ".fix",
            ]),
        None,
    )?;
    repository.inspect_tags(&tags)?;
    if sign {
        repository.sign_tags(&tags)?;
    } else {
        repository.index_trusted_tags()?;
    }
    Ok(())
}
