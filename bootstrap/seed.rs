//! Writes a seed repository whose certificates are issued directly on Linux.
//!
//! usage: seed create DIRECTORY COMPILER WASM2C CLANG TOOLCHAIN [NAME=ELF | --blob NAME=FILE | --directory NAME=PATH]...
//!        seed add DIRECTORY [NAME=ELF | --blob NAME=FILE | --directory NAME=PATH]...
//!
//! The compiler wrapper ELF is the trusted author. Every ELF becomes a Runnable
//! it authors, and `compile` closes the wrapper over its Bootstrap resources.
//! `add` issues more Runnables authored by an existing seed's `trusted-compiler`.

use fixhandle::{Handle, Object, Tree};
use fixrepository::Repository;
use std::{env, fs, path::Path};

#[derive(Debug)]
struct Failure(String);

macro_rules! failure {
    ($($error:ty),*) => {
        $(impl From<$error> for Failure {
            fn from(error: $error) -> Self {
                Self(format!("{error:?}"))
            }
        })*
    };
}

failure!(
    std::io::Error,
    fixhandle::Error,
    fixrepository::RepositoryError,
    flatware_tree::Error
);

type Result<T> = std::result::Result<T, Failure>;

fn tag(repository: &Repository, fields: &[Handle]) -> Result<Handle> {
    let tag = Tree::canonical(fields)?.tag_descriptor();
    let bytes: Vec<u8> = fields.iter().flat_map(|field| *field.as_bytes()).collect();
    repository.publish(tag.into(), &bytes)?;
    Ok(tag.into())
}

fn certificate(
    repository: &Repository,
    author: Handle,
    body: Handle,
    claim: &str,
) -> Result<Handle> {
    let claim = repository.create_blob(claim.as_bytes())?.into();
    tag(repository, &[author, body, claim])
}

fn runnable(repository: &Repository, author: Handle, path: &str) -> Result<Handle> {
    let executable = repository.create_blob(&fs::read(path)?)?.into();
    certificate(repository, author, executable, "Runnable")
}

fn populate(repository: &Repository, author: Handle, entries: &[String]) -> Result<()> {
    let mut entries = entries.iter();
    while let Some(entry) = entries.next() {
        let directory = entry == "--directory";
        let blob = entry == "--blob";
        let entry = if directory || blob {
            entries.next().ok_or(Failure(USAGE.into()))?
        } else {
            entry
        };
        let (name, path) = entry
            .split_once('=')
            .ok_or(Failure("expected NAME=PATH".into()))?;
        let value = if directory {
            if !fs::metadata(path)?.is_dir() {
                return Err(Failure("expected a directory".into()));
            }
            flatware_tree::import_contents(
                repository,
                Path::new(path),
                flatware_tree::Symlinks::Follow,
            )?
        } else if blob {
            repository.create_blob(&fs::read(path)?)?.into()
        } else {
            runnable(repository, author, path)?
        };
        repository.write_label(name, value)?;
    }
    Ok(())
}

fn create(arguments: &[String]) -> Result<()> {
    let [directory, compiler, wasm2c, clang, toolchain, rest @ ..] = arguments else {
        return Err(Failure(USAGE.into()));
    };
    let path = Path::new(directory).join(".fix");
    let repository = Repository::init(
        path.to_str()
            .ok_or(Failure("directory is not UTF-8".into()))?,
    )?;
    let compiler = repository.create_blob(&fs::read(compiler)?)?;
    let author: Handle = Object::try_from(Handle::from(compiler))?.into_ref().into();
    let fixpoint = certificate(&repository, author, compiler.into(), "Runnable")?;
    let wasm2c = runnable(&repository, author, wasm2c)?;
    let clang = runnable(&repository, author, clang)?;
    let toolchain = flatware_tree::import(
        &repository,
        Path::new(toolchain),
        b"/toolchain",
        flatware_tree::Symlinks::Follow,
    )?;
    let resources = repository.create_tree(&[wasm2c, clang, toolchain.into()])?;
    let resources = certificate(&repository, author, resources.into(), "Bootstrap")?;
    let limits = repository.create_blob(b"")?.into();
    let compile = repository.create_tree(&[limits, fixpoint, resources])?;
    repository.write_label("trusted-compiler", author)?;
    repository.write_label("compile", compile.into())?;
    repository.write_label("wasm2c", wasm2c)?;
    repository.write_label("clang", clang)?;
    populate(&repository, author, rest)?;
    fs::write(path.join("trust"), format!("{author}\n"))?;
    Ok(())
}

fn add(arguments: &[String]) -> Result<()> {
    let [directory, rest @ ..] = arguments else {
        return Err(Failure(USAGE.into()));
    };
    let path = Path::new(directory).join(".fix");
    let repository = Repository::open(
        path.to_str()
            .ok_or(Failure("directory is not UTF-8".into()))?,
    )?;
    let author = repository.read_label("trusted-compiler")?;
    populate(&repository, author, rest)
}

const USAGE: &str = "usage: seed create DIRECTORY COMPILER WASM2C CLANG TOOLCHAIN [NAME=ELF | --blob NAME=FILE | --directory NAME=PATH]...\n       seed add DIRECTORY [NAME=ELF | --blob NAME=FILE | --directory NAME=PATH]...";

fn main() -> std::process::ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let result = match arguments.split_first() {
        Some((command, rest)) if command == "create" => create(rest),
        Some((command, rest)) if command == "add" => add(rest),
        _ => Err(Failure(USAGE.into())),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(Failure(message)) => {
            eprintln!("seed: {message}");
            std::process::ExitCode::FAILURE
        }
    }
}
