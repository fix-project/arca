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
    Scheduler(&'static str),
    Repository(RepositoryError),
    Handle(Error),
    Io(ErrorKind),
    Trust(crate::arca::trust::TrustError),
}
impl core::fmt::Display for CliError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Trust(error) => write!(f, "{error}"),
            Self::Usage(message) | Self::Scheduler(message) => f.write_str(message),
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

fn stdin() -> Result<Vec<u8>, CliError> {
    Ok(File::stdio(StandardStream::Stdin)?.try_read_to_end()?)
}

fn integer_width(format: &str) -> Result<usize, CliError> {
    match format {
        "--u8" => Ok(1),
        "--u32" => Ok(4),
        "--u64" => Ok(8),
        _ => Err(CliError::Usage("expected --string, --u8, --u32, or --u64")),
    }
}

fn blob_bytes(arguments: &[String]) -> Result<Vec<u8>, CliError> {
    if let [path] = arguments {
        return if path == "-" {
            stdin()
        } else {
            Ok(kernel::host::fs::read(path)?)
        };
    }
    let [format, value] = arguments else {
        return Err(CliError::Usage("expected a Blob format and value"));
    };
    if format == "--string" {
        return Ok(value.as_bytes().to_vec());
    }
    let width = integer_width(format)?;
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(CliError::Usage("expected an unsigned decimal integer"));
    }
    let number = value
        .parse::<u64>()
        .map_err(|_| CliError::Usage("integer exceeds format range"))?;
    if width < 8 && number >= 1 << (width * 8) {
        return Err(CliError::Usage("integer exceeds format range"));
    }
    Ok(number.to_le_bytes()[..width].to_vec())
}

fn values(arguments: &[String]) -> Result<Vec<String>, CliError> {
    if arguments.len() != 1 || arguments[0] != "-" {
        return Ok(arguments.to_vec());
    }
    let bytes = stdin()?;
    let text = core::str::from_utf8(&bytes)
        .map_err(|_| CliError::Usage("expected UTF-8 handle tokens"))?;
    Ok(text.split_whitespace().map(String::from).collect())
}

fn resolve(repository: &Repository, value: &str) -> Result<Handle, CliError> {
    if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        let mut bytes = [0; 32];
        hex::decode_to_slice(value, &mut bytes)
            .map_err(|_| CliError::Usage("invalid hexadecimal handle"))?;
        Ok(repository.lookup(&bytes)?)
    } else {
        Ok(repository.read_label(value)?)
    }
}

fn reference(value: Handle) -> Result<Ref, Error> {
    match value.view() {
        HandleView::Object(object) => Ok(object.into_ref()),
        _ => Ref::try_from(value),
    }
}

fn construct(command: &str, value: Handle) -> Result<Handle, CliError> {
    Ok(match command {
        "create-ref" => Object::try_from(value)?.into_ref().into(),
        "create-identification-thunk" => reference(value)?.identification().into(),
        "create-application-thunk" => TreeRef::try_from(Handle::from(reference(value)?))?
            .application()
            .into(),
        "create-selection-thunk" => TreeRef::try_from(Handle::from(reference(value)?))?
            .selection()
            .into(),
        "create-strict-encode" => Thunk::try_from(value)?.strict().into(),
        "create-shallow-encode" => Thunk::try_from(value)?.shallow().into(),
        _ => return Err(CliError::Usage("unknown command")),
    })
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

pub fn run(mut arguments: &[String]) -> Result<u8, CliError> {
    let trust = if let [flag, contents, rest @ ..] = arguments
        && flag == "--trust"
    {
        arguments = rest;
        crate::arca::trust::Trust::parse(contents).map_err(CliError::Trust)?
    } else {
        crate::arca::trust::Trust::default()
    };
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(CliError::Usage("expected a command"));
    };
    let mut arguments = arguments[1..].to_vec();
    let show_labels =
        command == "read-tree" && arguments.first().is_some_and(|arg| arg == "--labels");
    if show_labels {
        arguments.remove(0);
    }
    let mut label = None;
    if command.starts_with("create-")
        && arguments.len() >= 2
        && arguments[arguments.len() - 2] == "-l"
    {
        label = arguments.pop();
        arguments.pop();
    }
    if command == "init" {
        if !arguments.is_empty() {
            return Err(CliError::Usage("init takes no arguments"));
        }
        Repository::init(".fix")?;
        return Ok(0);
    }
    let repository = Repository::discover(".")?;
    let mut output = File::stdio(StandardStream::Stdout)?;
    if command == "repository" {
        if !arguments.is_empty() {
            return Err(CliError::Usage("repository takes no arguments"));
        }
        output.try_write_all(format!("{}\n", repository.path()).as_bytes())?;
        return Ok(0);
    }
    if command == "user_scheduler" {
        let [value, scheduler, workers] = arguments.as_slice() else {
            return Err(CliError::Usage("expected VALUE SCHEDULER_ELF WORKERS"));
        };
        let workers = workers
            .parse::<usize>()
            .map_err(|_| CliError::Usage("worker count must be 1..=256"))?;
        if !(1..=256).contains(&workers) {
            return Err(CliError::Usage("worker count must be 1..=256"));
        }
        let root = resolve(&repository, value)?;
        let elf = kernel::host::fs::read(scheduler)?;
        let storage = alloc::sync::Arc::new(crate::storage::repository::RepositoryStorage::new(
            repository,
        ));
        let runtime = alloc::sync::Arc::new(crate::arca::FixOnArca::new(storage.clone()));
        let report = crate::user_scheduler_driver::start(runtime, elf.into(), root, workers)
            .map_err(CliError::Scheduler)?;
        let result = storage.save(report.result)?;
        output.try_write_all(format!("{result}\n").as_bytes())?;
        return Ok(0);
    }
    if command == "eval" || command == "think" {
        let memoize = command == "eval" && arguments.first().is_some_and(|arg| arg == "--memoize");
        if memoize {
            arguments.remove(0);
        }
        let arguments = values(&arguments)?;
        if arguments.len() != 1 {
            return Err(CliError::Usage("expected exactly one value"));
        }
        let value = resolve(&repository, &arguments[0])?;
        let storage = alloc::sync::Arc::new(crate::storage::repository::RepositoryStorage::new(
            repository,
        ));
        let runtime = if memoize {
            crate::arca::FixOnArca::memoized(storage.clone())
        } else {
            crate::arca::FixOnArca::new(storage.clone())
        }
        .with_trust(trust);
        let result = if command == "think" {
            crate::Evaluator::new(runtime).think(Thunk::try_from(value)?)
        } else if kernel::ncores() == 1 {
            crate::Evaluator::new(runtime).eval(value)
        } else {
            let evaluator = crate::parallel_evaluator::Evaluator::new(runtime, kernel::ncores());
            let result = evaluator.eval(value);
            evaluator.shutdown();
            result
        };
        let result = storage.save(result)?;
        output.try_write_all(format!("{result}\n").as_bytes())?;
        return Ok(0);
    }
    if command == "visit" {
        let paths = arguments.first().is_some_and(|arg| arg == "--paths");
        if paths {
            arguments.remove(0);
        }
        let arguments = values(&arguments)?;
        if arguments.len() != 1 {
            return Err(CliError::Usage("visit requires one value"));
        }
        let value = resolve(&repository, &arguments[0])?;
        if paths
            && !(arguments[0].len() == 64
                && arguments[0].bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            output.try_write_all(format!(".fix/labels/{}\0", arguments[0]).as_bytes())?;
        }
        repository.visit(value, |object| {
            if paths {
                for path in repository.object_paths(object)? {
                    output.try_write_all(format!("{path}\0").as_bytes())?;
                }
            } else {
                output.try_write_all(format!("{}\n", object.as_handle()).as_bytes())?;
            }
            Ok(())
        })?;
        return Ok(0);
    }
    if command == "read-blob" {
        let format = if arguments
            .first()
            .is_some_and(|value| value.starts_with("--"))
        {
            Some(arguments.remove(0))
        } else {
            None
        };
        let arguments = values(&arguments)?;
        if arguments.len() != 1 {
            return Err(CliError::Usage("read-blob requires one value"));
        }
        let blob = Blob::try_from(resolve(&repository, &arguments[0])?)?;
        let bytes = repository.read_blob(blob)?;
        match format.as_deref() {
            None => output.try_write_all(&bytes)?,
            Some("--string") => {
                core::str::from_utf8(&bytes)
                    .map_err(|_| CliError::Usage("Blob is not UTF-8 text"))?;
                output.try_write_all(&bytes)?;
            }
            Some(format) => {
                let width = integer_width(format)?;
                if bytes.len() != width {
                    return Err(CliError::Usage("Blob length does not match integer format"));
                }
                let mut integer = [0; 8];
                integer[..width].copy_from_slice(&bytes);
                output.try_write_all(format!("{}\n", u64::from_le_bytes(integer)).as_bytes())?;
            }
        }
        return Ok(0);
    }
    let created = match command {
        "create-blob" => {
            let paths = if let [format, path] = arguments.as_slice()
                && format == "--files-from"
            {
                let bytes = blob_bytes(core::slice::from_ref(path))?;
                let text = core::str::from_utf8(&bytes)
                    .map_err(|_| CliError::Usage("expected UTF-8 filenames"))?;
                if !text.is_empty() && !text.ends_with('\0') {
                    return Err(CliError::Usage("expected NUL-terminated filenames"));
                }
                text.split_terminator('\0')
                    .map(String::from)
                    .collect::<Vec<_>>()
            } else if arguments.first().is_some_and(|arg| arg.starts_with("--")) {
                Vec::new()
            } else {
                arguments.clone()
            };
            if !paths.is_empty() || arguments.first().is_some_and(|arg| arg == "--files-from") {
                if label.is_some() && paths.len() != 1 {
                    return Err(CliError::Usage("a label requires exactly one Blob"));
                }
                for path in paths {
                    let blob = repository.create_blob(&blob_bytes(&[path])?)?;
                    if let Some(name) = &label {
                        repository.write_label(name, blob.into())?;
                    }
                    output.try_write_all(format!("{}\n", blob.as_handle()).as_bytes())?;
                }
                return Ok(0);
            }
            Some(repository.create_blob(&blob_bytes(&arguments)?)?.into())
        }
        "create-tree" => {
            if arguments.as_slice() == ["--bindings", "-"] {
                let bytes = stdin()?;
                let text = core::str::from_utf8(&bytes)
                    .map_err(|_| CliError::Usage("expected UTF-8 name/value pairs"))?;
                if !text.is_empty() && !text.ends_with('\0') {
                    return Err(CliError::Usage("expected NUL-terminated name/value pairs"));
                }
                let fields: Vec<_> = text.split_terminator('\0').collect();
                if fields.len() % 2 != 0 {
                    return Err(CliError::Usage("expected name/value pairs"));
                }
                let mut bindings = Vec::new();
                for pair in fields.as_chunks::<2>().0 {
                    let name = repository.create_blob(pair[0].as_bytes())?;
                    let value = resolve(&repository, pair[1])?;
                    bindings.push(repository.create_tree(&[name.into(), value])?.into());
                }
                let tree = repository.create_tree(&bindings)?;
                if let Some(name) = &label {
                    repository.write_label(name, tree.into())?;
                }
                output.try_write_all(format!("{}\n", tree.as_handle()).as_bytes())?;
                return Ok(0);
            }
            let children = values(&arguments)?
                .iter()
                .map(|value| resolve(&repository, value))
                .collect::<Result<Vec<_>, _>>()?;
            Some(repository.create_tree(&children)?.into())
        }
        "label" => {
            match arguments.as_slice() {
                [action] if action == "list" => {
                    for name in repository.labels()? {
                        output.try_write_all(format!("{name}\n").as_bytes())?;
                    }
                }
                [action, name] if action == "get" => {
                    output
                        .try_write_all(format!("{}\n", repository.read_label(name)?).as_bytes())?;
                }
                [action, name, value] if action == "set" => {
                    let values = values(core::slice::from_ref(value))?;
                    let [value] = values.as_slice() else {
                        return Err(CliError::Usage("label set requires one value"));
                    };
                    repository.write_label(name, resolve(&repository, value)?)?;
                }
                _ => {
                    return Err(CliError::Usage(
                        "expected label list, get NAME, or set NAME VALUE",
                    ));
                }
            }
            None
        }
        _ => {
            if !matches!(
                command,
                "read-tree"
                    | "resolve"
                    | "get-type"
                    | "get-data-type"
                    | "get-encode-type"
                    | "len"
                    | "is-tag"
                    | "is-comparable"
                    | "create-ref"
                    | "create-identification-thunk"
                    | "create-application-thunk"
                    | "create-selection-thunk"
                    | "create-strict-encode"
                    | "create-shallow-encode"
            ) {
                return Err(CliError::Usage("unknown command"));
            }
            let arguments = values(&arguments)?;
            if arguments.len() != 1 {
                return Err(CliError::Usage("expected exactly one value"));
            }
            let value = resolve(&repository, &arguments[0])?;
            match command {
                "read-tree" => {
                    let labels = if show_labels {
                        repository
                            .labels()?
                            .into_iter()
                            .map(|name| {
                                let value = repository.read_label(&name)?;
                                Ok((name, value))
                            })
                            .collect::<Result<Vec<_>, RepositoryError>>()?
                    } else {
                        Vec::new()
                    };
                    for child in repository.read_tree(Tree::try_from(value)?)?.iter() {
                        let mut line = format!("{child}");
                        for (name, value) in &labels {
                            if value.as_bytes() == child.as_bytes() {
                                line.push_str(&format!("\t{name:?}"));
                            }
                        }
                        line.push('\n');
                        output.try_write_all(line.as_bytes())?;
                    }
                }
                "resolve" => output.try_write_all(format!("{value}\n").as_bytes())?,
                "get-type" => {
                    let kind = match value.view() {
                        HandleView::Object(_) => "object",
                        HandleView::Ref(_) => "ref",
                        HandleView::Thunk(_) => "thunk",
                        HandleView::Encode(_) => "encode",
                    };
                    output.try_write_all(format!("{kind}\n").as_bytes())?;
                }
                "get-data-type" => {
                    let object = reference(value)?.object_descriptor();
                    let kind = match object.view() {
                        ObjectView::Blob(_) => "blob",
                        ObjectView::Tree(_) => "tree",
                    };
                    output.try_write_all(format!("{kind}\n").as_bytes())?;
                }
                "get-encode-type" => {
                    let kind = match Encode::try_from(value)?.view() {
                        EncodeView::Strict(_) => "strict",
                        EncodeView::Shallow(_) => "shallow",
                    };
                    output.try_write_all(format!("{kind}\n").as_bytes())?;
                }
                "len" => output.try_write_all(
                    format!("{}\n", reference(value)?.handle().checked_len()?).as_bytes(),
                )?,
                "is-tag" => {
                    return Ok(u8::from(
                        !Tree::try_from(value).is_ok_and(|tree| tree.is_tag()),
                    ));
                }
                "is-comparable" => return Ok(u8::from(!value.is_comparable())),
                _ => {
                    return {
                        let created = construct(command, value)?;
                        if let Some(label) = label {
                            repository.write_label(&label, created)?;
                        }
                        output.try_write_all(format!("{created}\n").as_bytes())?;
                        Ok(0)
                    };
                }
            }
            None
        }
    };
    if let Some(created) = created {
        if let Some(label) = label {
            repository.write_label(&label, created)?;
        }
        output.try_write_all(format!("{created}\n").as_bytes())?;
    }
    Ok(0)
}
