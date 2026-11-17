//! Writes a seed repository whose certificates are issued directly on Linux.
//!
//! usage: seed create DIRECTORY COMPILER WASM2C CLANG TOOLCHAIN [NAME=ELF | --blob NAME=FILE | --directory NAME=PATH]...
//!        seed add DIRECTORY [NAME=ELF | --blob NAME=FILE | --directory NAME=PATH]...
//!        seed promote DIRECTORY RESULT_HANDLE_FILE
//!
//! The compiler wrapper ELF is the trusted author. Every ELF becomes a Runnable
//! it authors, and `compile` closes the wrapper over its Bootstrap resources.
//! `add` issues more Runnables authored by an existing seed's `trusted-compiler`.

use fixhandle::{Blob, Handle, Object, Tree};
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
    )?
    .unsigned();
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
    )?
    .unsigned();
    let author = repository.read_label("trusted-compiler")?;
    populate(&repository, author, rest)
}

fn read_handle(repository: &Repository, path: &str) -> Result<Handle> {
    let name = fs::read_to_string(path)?;
    let name = name.trim();
    if name.len() != 64 {
        return Err(Failure("invalid compilation result handle".into()));
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&name[index * 2..index * 2 + 2], 16)
            .map_err(|_| Failure("invalid compilation result handle".into()))?;
    }
    Ok(repository.lookup(&bytes)?)
}

fn promote(arguments: &[String]) -> Result<()> {
    let [directory, result] = arguments else {
        return Err(Failure(USAGE.into()));
    };
    let path = Path::new(directory).join(".fix");
    let repository = Repository::open(
        path.to_str()
            .ok_or(Failure("directory is not UTF-8".into()))?,
    )?
    .unsigned();
    let result = read_handle(&repository, result)?;
    let entries = repository.read_tree(Tree::try_from(result)?)?;
    let mut compiler = None;
    for entry in entries.iter() {
        let fields = repository.read_tree(Tree::try_from(*entry)?)?;
        let name = repository.read_blob(Blob::try_from(
            *fields
                .first()
                .ok_or(Failure("invalid compilation entry".into()))?,
        )?)?;
        if name.as_ref() == b"compile" {
            compiler = Some(
                *fields
                    .get(1)
                    .ok_or(Failure("invalid compilation entry".into()))?,
            );
            break;
        }
    }
    let compiled_closure = compiler.ok_or(Failure("compiled compiler is missing".into()))?;
    let compiled_closure = repository.read_tree(Tree::try_from(compiled_closure)?)?;
    let compiler = *compiled_closure
        .get(1)
        .ok_or(Failure("invalid compiled compiler closure".into()))?;
    let certificate_fields = repository.read_tree(Tree::try_from(compiler)?)?;
    if certificate_fields.len() != 3
        || repository
            .read_blob(Blob::try_from(certificate_fields[2])?)?
            .as_ref()
            != b"Runnable"
    {
        return Err(Failure("compiled compiler is not Runnable".into()));
    }
    let executable = certificate_fields[1];
    let author: Handle = Object::try_from(executable)?.into_ref().into();
    let runnable = certificate(&repository, author, executable, "Runnable")?;
    let previous = repository.read_tree(Tree::try_from(repository.read_label("compile")?)?)?;
    let limits = *previous
        .first()
        .ok_or(Failure("invalid compiler closure".into()))?;
    let bootstrap = *previous
        .get(2)
        .ok_or(Failure("invalid compiler closure".into()))?;
    let resources = repository.read_tree(Tree::try_from(bootstrap)?)?;
    let resource_body = *resources
        .get(1)
        .ok_or(Failure("invalid Bootstrap tag".into()))?;
    let resources = certificate(&repository, author, resource_body, "Bootstrap")?;
    let compile = repository.create_tree(&[limits, runnable, resources])?;
    repository.write_label("compile", compile.into())?;
    repository.write_label("trusted-compiler", author)?;
    let trust = path.join("trust");
    let mut trusted = fs::read_to_string(&trust)?;
    if !trusted.lines().any(|line| line == author.to_string()) {
        trusted.push_str(&format!("{author}\n"));
        fs::write(trust, trusted)?;
    }
    Ok(())
}

const USAGE: &str = "usage: seed create DIRECTORY COMPILER WASM2C CLANG TOOLCHAIN [NAME=ELF | --blob NAME=FILE | --directory NAME=PATH]...\n       seed add DIRECTORY [NAME=ELF | --blob NAME=FILE | --directory NAME=PATH]...\n       seed promote DIRECTORY RESULT_HANDLE_FILE";

fn main() -> std::process::ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let result = match arguments.split_first() {
        Some((command, rest)) if command == "create" => create(rest),
        Some((command, rest)) if command == "add" => add(rest),
        Some((command, rest)) if command == "promote" => promote(rest),
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
