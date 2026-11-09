extern crate alloc;

use crate::{
    handle::*,
    repository::{Repository, RepositoryError},
};
use alloc::{format, string::String, vec::Vec};
use common::protocol::control::{ErrorKind, StandardStream};
use kernel::host::fs::File;

#[derive(Debug)]
pub enum CliError {
    Usage(&'static str),
    Backend(String),
    Repository(RepositoryError),
    Handle(Error),
    Io(ErrorKind),
    Trust(crate::arca::trust::TrustError),
}
impl core::fmt::Display for CliError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Trust(error) => write!(f, "{error}"),
            Self::Backend(error) => f.write_str(error),
            Self::Usage(message) => f.write_str(message),
            Self::Repository(RepositoryError::NotInitialized) => {
                f.write_str("no Fix repository found; run 'fix init'")
            }
            Self::Repository(RepositoryError::InvalidRepository) => {
                f.write_str("invalid Fix repository")
            }
            Self::Repository(RepositoryError::InvalidLabel) => f.write_str("invalid label name"),
            Self::Repository(RepositoryError::MissingLabel { name, repository }) => {
                write!(f, "label {name:?} not found in repository {repository:?}")
            }
            Self::Repository(RepositoryError::NonCanonical) => {
                f.write_str("expected a canonical handle")
            }
            Self::Repository(RepositoryError::Io(ErrorKind::NotFound)) => {
                f.write_str("value or label is not present in this repository")
            }
            Self::Handle(Error::WrongType)
            | Self::Repository(RepositoryError::Handle(Error::WrongType)) => {
                f.write_str("value has the wrong Fix type")
            }
            Self::Io(error) | Self::Repository(RepositoryError::Io(error)) => {
                write!(f, "I/O error: {error:?}")
            }
            Self::Handle(error) | Self::Repository(RepositoryError::Handle(error)) => {
                write!(f, "invalid handle: {error:?}")
            }
            Self::Repository(RepositoryError::Storage(error)) => {
                write!(f, "store error: {error:?}")
            }
        }
    }
}
impl From<RepositoryError> for CliError {
    fn from(error: RepositoryError) -> Self {
        Self::Repository(error)
    }
}
impl From<Error> for CliError {
    fn from(error: Error) -> Self {
        Self::Handle(error)
    }
}
impl From<ErrorKind> for CliError {
    fn from(error: ErrorKind) -> Self {
        Self::Io(error)
    }
}

pub fn configure_directory(mut arguments: &[String]) -> Result<&[String], CliError> {
    while arguments.first().is_some_and(|argument| argument == "-C") {
        let directory = arguments
            .get(1)
            .ok_or(CliError::Usage("-C requires a directory"))?;
        kernel::host::os::set_current_dir(directory)?;
        arguments = &arguments[2..];
    }
    Ok(arguments)
}

/// Writes the handle of the result, or of the `Fix.error` request that replaced it, and
/// returns the exit status saying which.
fn report(
    backend: &crate::api::Backend,
    outcome: crate::user_scheduler_driver::Outcome,
) -> Result<(u8, String), CliError> {
    let (value, status) = match outcome {
        Ok(value) => (value, 0),
        Err(request) => (request.into(), fixapi::FIX_ERROR_STATUS),
    };
    let value = backend.save(value)?;
    Ok((status, format!("{value}\n")))
}

pub fn run(arguments: &[String]) -> Result<u8, CliError> {
    let (status, output) = execute(arguments, &mut None)?;
    File::stdio(StandardStream::Stdout)?.try_write_all(output.as_bytes())?;
    Ok(status)
}

fn execute(
    mut arguments: &[String],
    retained: &mut Option<alloc::sync::Arc<crate::storage::repository::RepositoryStorage>>,
) -> Result<(u8, String), CliError> {
    let mut trust = if let [flag, contents, rest @ ..] = arguments
        && flag == "--trust"
    {
        arguments = rest;
        crate::arca::trust::Trust::parse(contents).map_err(CliError::Trust)?
    } else {
        crate::arca::trust::Trust::default()
    };
    let profile = if arguments.first().is_some_and(|arg| arg == "--profile") {
        arguments = &arguments[1..];
        true
    } else {
        false
    };
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(CliError::Usage("expected a command"));
    };
    let mut arguments = arguments[1..].to_vec();
    let repository = Repository::discover(".")?;
    let mut memoize = fixapi::Memoization::Auto;
    if let Some(index) = arguments.iter().position(|arg| arg == "--memoize") {
        memoize = arguments
            .get(index + 1)
            .ok_or(CliError::Usage("missing memoization mode"))?
            .parse()
            .map_err(CliError::Usage)?;
        arguments.drain(index..index + 2);
    }
    let storage = retained
        .get_or_insert_with(|| {
            alloc::sync::Arc::new(crate::storage::repository::RepositoryStorage::new(
                repository,
            ))
        })
        .clone();
    let repository = storage.repository();
    if command == "call" {
        let [combination] = arguments.as_slice() else {
            return Err(CliError::Usage("expected a callable combination"));
        };
        let combination = Tree::try_from(crate::api::resolve(repository, combination)?)?;
        let backend = crate::api::Backend::new(storage, trust, memoize).with_profile(profile);
        return report(&backend, backend.call(combination));
    }
    if command == "schedule" {
        let mut taggers = Vec::new();
        while arguments
            .first()
            .is_some_and(|value| value == "--trusted-tagger")
        {
            let name = arguments
                .get(1)
                .ok_or(CliError::Usage("--trusted-tagger requires a callable"))?;
            taggers.push(Tree::try_from(crate::api::resolve(repository, name)?)?);
            arguments.drain(..2);
        }
        let mut todos = Vec::new();
        while arguments.first().is_some_and(|value| value == "--todo") {
            let [_, name, uri, ..] = arguments.as_slice() else {
                return Err(CliError::Usage("--todo requires a reference and URI"));
            };
            if uri.is_empty() || uri.chars().any(char::is_control) {
                return Err(CliError::Usage("invalid Todo URI"));
            }
            todos.push((
                crate::api::reference(crate::api::resolve(repository, name)?)?,
                repository.create_blob(uri.as_bytes())?,
            ));
            arguments.drain(..3);
        }
        let [combination] = arguments.as_slice() else {
            return Err(CliError::Usage("expected a scheduler combination"));
        };
        let combination = Tree::try_from(crate::api::resolve(repository, combination)?)?;
        for tagger in taggers {
            trust
                .add_tagger(&*storage, tagger)
                .map_err(CliError::Trust)?;
        }
        let backend = crate::api::Backend::new(storage, trust, memoize).with_profile(profile);
        let result = backend
            .schedule(combination, todos)
            .map_err(CliError::Backend)?;
        return report(&backend, result);
    }
    if command == "evaluate" {
        let [scheduler, collector, value] = arguments.as_slice() else {
            return Err(CliError::Usage("expected scheduler, collector, and input"));
        };
        let scheduler = Tree::try_from(crate::api::resolve(repository, scheduler)?)?;
        let collector = Tree::try_from(crate::api::resolve(repository, collector)?)?;
        let value = crate::api::resolve(repository, value)?;
        trust
            .add_tagger(&*storage, collector)
            .map_err(CliError::Trust)?;
        let backend = crate::api::Backend::new(storage, trust, memoize).with_profile(profile);
        let result = backend
            .eval(value, scheduler, collector)
            .map_err(CliError::Backend)?;
        return report(&backend, result);
    }
    Err(CliError::Usage("expected call, schedule, or evaluate"))
}

pub fn serve() -> Result<(), CliError> {
    use common::{BuddyAllocator, buffer::Buffer};
    let mut input = File::stdio(StandardStream::Stdin)?;
    let mut output = File::stdio(StandardStream::Stdout)?;
    fn read(input: &mut File, size: usize) -> Result<Option<Vec<u8>>, CliError> {
        let mut bytes = alloc::vec::Vec::with_capacity_in(size, BuddyAllocator);
        bytes.resize(size, 0);
        let mut buffer = Buffer::from(bytes);
        let mut offset = 0;
        while offset < size {
            let n = input.try_read(&mut buffer.slice_mut(offset..))?;
            if n == 0 {
                return if offset == 0 {
                    Ok(None)
                } else {
                    Err(CliError::Usage("truncated session request"))
                };
            }
            offset += n;
        }
        Ok(Some(buffer.to_vec()))
    }
    let mut storage = None;
    while let Some(length) = read(&mut input, 4)? {
        let length = u32::from_le_bytes(length.try_into().unwrap()) as usize;
        let frame =
            read(&mut input, length)?.ok_or(CliError::Usage("truncated session request"))?;
        let (reuse, mut remaining) = frame
            .split_first()
            .ok_or(CliError::Usage("missing session storage flag"))?;
        match reuse {
            0 => storage = None,
            1 => {}
            _ => return Err(CliError::Usage("invalid session storage flag")),
        }
        let mut arguments = Vec::new();
        while !remaining.is_empty() {
            let (length, rest) = remaining
                .split_first_chunk::<4>()
                .ok_or(CliError::Usage("truncated session argument"))?;
            let length = u32::from_le_bytes(*length) as usize;
            let (bytes, rest) = rest
                .split_at_checked(length)
                .ok_or(CliError::Usage("truncated session argument"))?;
            arguments.push(
                String::from_utf8(bytes.to_vec())
                    .map_err(|_| CliError::Usage("invalid session argument"))?,
            );
            remaining = rest;
        }
        let (status, response) = if arguments.is_empty() {
            (0, String::new())
        } else {
            execute(&arguments, &mut storage).unwrap_or_else(|error| (2, format!("{error}")))
        };
        let mut frame = Vec::with_capacity(5 + response.len());
        frame.push(status);
        frame.extend_from_slice(&(response.len() as u32).to_le_bytes());
        frame.extend_from_slice(response.as_bytes());
        output.try_write_all(&frame)?;
    }
    Ok(())
}
