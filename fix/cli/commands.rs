extern crate alloc;

use alloc::{format, string::String, vec::Vec};
use clap::{Args, Subcommand};
use common::protocol::control::{ErrorKind, IoErrorKind};
use fixhandle::*;
use fixrepository::{Repository, RepositoryError};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum CliError {
    Usage(&'static str),
    Backend(String),
    Repository(RepositoryError),
    Handle(Error),
    Io(ErrorKind),
}
impl core::fmt::Display for CliError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
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
impl std::error::Error for CliError {}
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
    let mut bytes = Vec::new();
    std::io::stdin().read_to_end(&mut bytes).map_err(io_error)?;
    Ok(bytes)
}

/// A handle or label name; `-` reads whitespace-separated tokens from stdin.
fn values(arguments: &[String]) -> Result<Vec<String>, CliError> {
    if arguments.len() != 1 || arguments[0] != "-" {
        return Ok(arguments.to_vec());
    }
    let bytes = stdin()?;
    let text = core::str::from_utf8(&bytes)
        .map_err(|_| CliError::Usage("expected UTF-8 handle tokens"))?;
    Ok(text.split_whitespace().map(String::from).collect())
}

fn single(value: &str) -> Result<String, CliError> {
    let mut values = values(&[value.into()])?;
    if values.len() != 1 {
        return Err(CliError::Usage("expected exactly one value"));
    }
    Ok(values.remove(0))
}

fn value(repository: &Repository, operand: Operand) -> Result<Handle, CliError> {
    resolve(repository, &single(&operand.value)?)
}

fn file_bytes(path: &str) -> Result<Vec<u8>, CliError> {
    if path == "-" {
        stdin()
    } else {
        Ok(std::fs::read(path).map_err(io_error)?)
    }
}

pub(crate) fn resolve(repository: &Repository, value: &str) -> Result<Handle, CliError> {
    Ok(crate::resolve(repository, value)?)
}

fn reference(value: Handle) -> Result<Ref, Error> {
    crate::reference(value)
}

/// A value operand: a handle or label name, or `-` to read one from stdin.
#[derive(Args, Debug)]
pub struct Operand {
    #[arg(value_name = "VALUE")]
    pub value: String,
}

/// The encoding of a Blob read back as text.
#[derive(Args, Debug)]
#[group(multiple = false)]
pub struct BlobFormat {
    /// Require UTF-8 text.
    #[arg(long)]
    string: bool,
    /// Print a one-byte little-endian integer.
    #[arg(long)]
    u8: bool,
    /// Print a four-byte little-endian integer.
    #[arg(long)]
    u32: bool,
    /// Print an eight-byte little-endian integer.
    #[arg(long)]
    u64: bool,
}

/// The contents of new Blobs.
#[derive(Args, Debug)]
#[group(required = true, multiple = false)]
pub struct BlobSource {
    /// Store TEXT.
    #[arg(long, value_name = "TEXT", allow_hyphen_values = true)]
    string: Option<String>,
    /// Store a one-byte little-endian integer.
    #[arg(long, value_name = "N")]
    u8: Option<u8>,
    /// Store a four-byte little-endian integer.
    #[arg(long, value_name = "N")]
    u32: Option<u32>,
    /// Store an eight-byte little-endian integer.
    #[arg(long, value_name = "N")]
    u64: Option<u64>,
    /// Store each file named by the NUL-terminated paths in FILE (`-` for stdin).
    #[arg(long, value_name = "FILE")]
    files_from: Option<String>,
    /// Store each file (`-` for stdin).
    #[arg(value_name = "PATH")]
    paths: Vec<String>,
}

/// Label commands.
#[derive(Subcommand, Debug)]
pub enum LabelCommand {
    /// List label names.
    List,
    /// Print the handle a label names.
    Get { name: String },
    /// Point a label at a value.
    Set { name: String, value: String },
}

/// Commands whose value operands are handles or label names.
#[derive(Subcommand, Debug)]
pub enum Plumbing {
    /// Print the repository path.
    Repository,
    /// Import an archive into the repository.
    Import { archive: PathBuf },
    /// Import a directory as a Flatware tree.
    FlatwareImport { path: PathBuf, name: Option<String> },
    /// List, read, or write labels.
    #[command(subcommand)]
    Label(LabelCommand),
    /// Print a value's handle.
    Resolve(Operand),
    /// Write a Blob's contents to stdout.
    ReadBlob {
        #[command(flatten)]
        format: BlobFormat,
        value: String,
    },
    /// Print a Tree's children.
    ReadTree {
        /// Also print the labels that name each child.
        #[arg(long)]
        labels: bool,
        value: String,
    },
    /// Print an Object's or Ref's length.
    Len(Operand),
    /// Print object, ref, thunk, or encode.
    GetType(Operand),
    /// Print blob or tree.
    GetDataType(Operand),
    /// Print strict or shallow.
    GetEncodeType(Operand),
    /// Exit 0 if the value is a Tag.
    IsTag(Operand),
    /// Exit 0 if the value is Comparable.
    IsComparable(Operand),
    /// Create Blobs and print their handles.
    CreateBlob(BlobSource),
    /// Create a Tree and print its handle.
    CreateTree {
        /// Read NUL-terminated name/value pairs from stdin and create a Tree of bindings.
        #[arg(long, value_name = "-", value_parser = ["-"], conflicts_with = "values")]
        bindings: Option<String>,
        /// Children; a single `-` reads them from stdin.
        values: Vec<String>,
    },
    /// Create a Ref to an Object.
    CreateRef(Operand),
    /// Create an identification Thunk.
    CreateIdentificationThunk(Operand),
    /// Create an application Thunk.
    CreateApplicationThunk(Operand),
    /// Create a selection Thunk.
    CreateSelectionThunk(Operand),
    /// Create a strict Encode.
    CreateStrictEncode(Operand),
    /// Create a shallow Encode.
    CreateShallowEncode(Operand),
    /// Print the objects reachable from values.
    Visit {
        /// Print NUL-terminated repository paths instead of handles.
        #[arg(long)]
        paths: bool,
        /// Do not visit the data of VALUE.
        #[arg(long = "without-backing", value_name = "VALUE")]
        without_backing: Vec<String>,
        #[arg(required = true)]
        values: Vec<String>,
    },
    /// Apply a combination with the runtime directly.
    Call { combination: String },
    /// Run a scheduler combination.
    Schedule {
        /// Allow CALLABLE to create Tags without checking their author.
        #[arg(long = "trusted-tagger", value_name = "CALLABLE")]
        trusted_taggers: Vec<String>,
        /// Record that REFERENCE can be fetched from URI.
        #[arg(long, num_args = 2, value_names = ["REFERENCE", "URI"])]
        todo: Vec<String>,
        combination: String,
    },
}

impl Plumbing {
    /// Whether `-l` can label this command's result.
    pub fn creates(&self) -> bool {
        matches!(
            self,
            Self::FlatwareImport { .. }
                | Self::CreateBlob(_)
                | Self::CreateTree { .. }
                | Self::CreateRef(_)
                | Self::CreateIdentificationThunk(_)
                | Self::CreateApplicationThunk(_)
                | Self::CreateSelectionThunk(_)
                | Self::CreateStrictEncode(_)
                | Self::CreateShallowEncode(_)
        )
    }
}

pub fn init() -> Result<(), CliError> {
    Repository::init(".fix")?;
    Ok(())
}

fn print(output: &mut Output, handle: impl core::fmt::Display) -> Result<(), CliError> {
    Ok(output.try_write_all(format!("{handle}\n").as_bytes())?)
}

fn created(
    repository: &Repository,
    label: &Option<String>,
    handle: Handle,
) -> Result<(), CliError> {
    if let Some(label) = label {
        repository.write_label(label, handle)?;
    }
    print(&mut Output, handle)
}

/// Writes the objects reachable from `roots` to `archive`, or lists them.
pub fn visit(
    roots: &[String],
    without_backing: &[String],
    paths: bool,
    archive: Option<&Path>,
) -> Result<(), CliError> {
    let repository = Repository::discover(".")?;
    let mut output = Output;
    let without_backing = without_backing
        .iter()
        .map(|value| Ok(reference(resolve(&repository, value)?)?.object_descriptor()))
        .collect::<Result<Vec<_>, CliError>>()?;
    let paths = paths || archive.is_some();
    let mut handles = Vec::new();
    let mut listed = Vec::new();
    for argument in values(roots)? {
        handles.push(resolve(&repository, &argument)?);
        if paths && !(argument.len() == 64 && argument.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            listed.push(format!(".fix/labels/{argument}"));
        }
    }
    repository.visit(&handles, &without_backing, |object, include_backing| {
        if paths {
            listed.extend(repository.object_paths(object, include_backing)?);
        } else {
            output.try_write_all(format!("{}\n", object.as_handle()).as_bytes())?;
        }
        Ok(())
    })?;
    match archive {
        Some(archive) => crate::archive::export(&repository, listed, archive)?,
        None if paths => {
            for path in listed {
                output.try_write_all(format!("{path}\0").as_bytes())?;
            }
        }
        None => {}
    }
    Ok(())
}

pub fn flatware_export(value: &str, path: &Path) -> Result<(), CliError> {
    let repository = Repository::discover(".")?;
    let tree = Tree::try_from(resolve(&repository, value)?)?;
    flatware_tree::export(&repository, tree, path)
        .map_err(|error| CliError::Backend(error.to_string()))
}

fn backend(error: crate::Error) -> CliError {
    CliError::Backend(error.to_string())
}

fn execute(options: crate::Options, profile: bool, arguments: Vec<String>) -> Result<u8, CliError> {
    let mut client = crate::Client::open(Path::new("."), options).map_err(backend)?;
    client.set_profile(profile);
    let value = match client.execute(&arguments) {
        Ok(value) => value,
        Err(crate::Error::Execution(status)) => return Ok(status),
        Err(error) => return Err(backend(error)),
    };
    print(&mut Output, client.save(&value).map_err(backend)?)?;
    Ok(0)
}

pub fn run(
    command: Plumbing,
    label: Option<String>,
    options: crate::Options,
    profile: bool,
) -> Result<u8, CliError> {
    if label.is_some() && !command.creates() {
        return Err(CliError::Usage("-l labels an expression result"));
    }
    let repository = Repository::discover(".")?;
    let mut output = Output;
    match command {
        Plumbing::Repository => print(&mut output, repository.path())?,
        Plumbing::Import { archive } => crate::archive::import(&repository, &archive)?,
        Plumbing::FlatwareImport { path, name } => {
            let name = match name {
                Some(name) => name.into_bytes(),
                None => {
                    use std::os::unix::ffi::OsStrExt;
                    std::path::absolute(&path)
                        .map_err(io_error)?
                        .file_name()
                        .map(|name| name.as_bytes().to_vec())
                        .unwrap_or_default()
                }
            };
            let tree =
                flatware_tree::import(&repository, &path, &name, flatware_tree::Symlinks::Preserve)
                    .map_err(|error| CliError::Backend(error.to_string()))?;
            created(&repository, &label, tree.into())?;
        }
        Plumbing::Label(LabelCommand::List) => {
            for name in repository.labels()? {
                print(&mut output, name)?;
            }
        }
        Plumbing::Label(LabelCommand::Get { name }) => {
            print(&mut output, repository.read_label(&name)?)?;
        }
        Plumbing::Label(LabelCommand::Set { name, value }) => {
            let value = resolve(&repository, &single(&value)?)?;
            repository.write_label(&name, value)?;
        }
        Plumbing::ReadBlob { format, value } => {
            let blob = Blob::try_from(resolve(&repository, &single(&value)?)?)?;
            let bytes = repository.read_blob(blob)?;
            let width = [(format.u8, 1), (format.u32, 4), (format.u64, 8)]
                .into_iter()
                .find_map(|(selected, width)| selected.then_some(width));
            match width {
                Some(width) => {
                    if bytes.len() != width {
                        return Err(CliError::Usage("Blob length does not match integer format"));
                    }
                    let mut integer = [0; 8];
                    integer[..width].copy_from_slice(&bytes);
                    print(&mut output, u64::from_le_bytes(integer))?;
                }
                None => {
                    if format.string {
                        core::str::from_utf8(&bytes)
                            .map_err(|_| CliError::Usage("Blob is not UTF-8 text"))?;
                    }
                    output.try_write_all(&bytes)?;
                }
            }
        }
        Plumbing::ReadTree { labels, value } => {
            let value = resolve(&repository, &single(&value)?)?;
            let labels = if labels {
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
                print(&mut output, line)?;
            }
        }
        Plumbing::CreateBlob(source) => {
            let integer = [
                source.u8.map(|n| vec![n]),
                source.u32.map(|n| n.to_le_bytes().to_vec()),
                source.u64.map(|n| n.to_le_bytes().to_vec()),
            ]
            .into_iter()
            .flatten()
            .next();
            if let Some(bytes) = source.string.map(String::into_bytes).or(integer) {
                created(&repository, &label, repository.create_blob(&bytes)?.into())?;
                return Ok(0);
            }
            let paths = match source.files_from {
                Some(list) => {
                    let bytes = file_bytes(&list)?;
                    let text = core::str::from_utf8(&bytes)
                        .map_err(|_| CliError::Usage("expected UTF-8 filenames"))?;
                    if !text.is_empty() && !text.ends_with('\0') {
                        return Err(CliError::Usage("expected NUL-terminated filenames"));
                    }
                    text.split_terminator('\0').map(String::from).collect()
                }
                None => source.paths,
            };
            if label.is_some() && paths.len() != 1 {
                return Err(CliError::Usage("a label requires exactly one Blob"));
            }
            for path in paths {
                created(
                    &repository,
                    &label,
                    repository.create_blob(&file_bytes(&path)?)?.into(),
                )?;
            }
        }
        Plumbing::CreateTree {
            bindings: Some(_), ..
        } => {
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
            created(
                &repository,
                &label,
                repository.create_tree(&bindings)?.into(),
            )?;
        }
        Plumbing::CreateTree {
            bindings: None,
            values: children,
        } => {
            let children = values(&children)?
                .iter()
                .map(|value| resolve(&repository, value))
                .collect::<Result<Vec<_>, _>>()?;
            created(
                &repository,
                &label,
                repository.create_tree(&children)?.into(),
            )?;
        }
        Plumbing::Visit {
            paths,
            without_backing,
            values,
        } => {
            visit(&values, &without_backing, paths, None)?;
        }
        Plumbing::Call { combination } => {
            return execute(options, profile, vec!["call".into(), combination]);
        }
        Plumbing::Schedule {
            trusted_taggers,
            todo,
            combination,
        } => {
            let mut arguments = vec![
                "schedule".into(),
                "--memoize".into(),
                options.memoize.to_string(),
            ];
            for tagger in trusted_taggers {
                arguments.extend(["--trusted-tagger".into(), tagger]);
            }
            for pair in todo.chunks(2) {
                arguments.push("--todo".into());
                arguments.extend_from_slice(pair);
            }
            arguments.push(combination);
            return execute(options, profile, arguments);
        }
        Plumbing::Resolve(operand) => print(&mut output, value(&repository, operand)?)?,
        Plumbing::Len(operand) => {
            let value = value(&repository, operand)?;
            print(&mut output, reference(value)?.handle().checked_len()?)?;
        }
        Plumbing::GetType(operand) => {
            let kind = match value(&repository, operand)?.view() {
                HandleView::Object(_) => "object",
                HandleView::Ref(_) => "ref",
                HandleView::Thunk(_) => "thunk",
                HandleView::Encode(_) => "encode",
            };
            print(&mut output, kind)?;
        }
        Plumbing::GetDataType(operand) => {
            let object = reference(value(&repository, operand)?)?.object_descriptor();
            let kind = match object.view() {
                ObjectView::Blob(_) => "blob",
                ObjectView::Tree(_) => "tree",
            };
            print(&mut output, kind)?;
        }
        Plumbing::GetEncodeType(operand) => {
            let kind = match Encode::try_from(value(&repository, operand)?)?.view() {
                EncodeView::Strict(_) => "strict",
                EncodeView::Shallow(_) => "shallow",
            };
            print(&mut output, kind)?;
        }
        Plumbing::IsTag(operand) => {
            let value = value(&repository, operand)?;
            return Ok(u8::from(
                !Tree::try_from(value).is_ok_and(|tree| tree.is_tag()),
            ));
        }
        Plumbing::IsComparable(operand) => {
            return Ok(u8::from(!value(&repository, operand)?.is_comparable()));
        }
        Plumbing::CreateRef(operand) => {
            let created_ref = Object::try_from(value(&repository, operand)?)?.into_ref();
            created(&repository, &label, created_ref.into())?;
        }
        Plumbing::CreateIdentificationThunk(operand) => {
            let thunk = reference(value(&repository, operand)?)?.identification();
            created(&repository, &label, thunk.into())?;
        }
        Plumbing::CreateApplicationThunk(operand) => {
            let tree = TreeRef::try_from(Handle::from(reference(value(&repository, operand)?)?))?;
            created(&repository, &label, tree.application().into())?;
        }
        Plumbing::CreateSelectionThunk(operand) => {
            let tree = TreeRef::try_from(Handle::from(reference(value(&repository, operand)?)?))?;
            created(&repository, &label, tree.selection().into())?;
        }
        Plumbing::CreateStrictEncode(operand) => {
            let thunk = Thunk::try_from(value(&repository, operand)?)?;
            created(&repository, &label, thunk.strict().into())?;
        }
        Plumbing::CreateShallowEncode(operand) => {
            let thunk = Thunk::try_from(value(&repository, operand)?)?;
            created(&repository, &label, thunk.shallow().into())?;
        }
    }
    Ok(0)
}
fn io_error(error: std::io::Error) -> ErrorKind {
    IoErrorKind::from(error.kind()).into()
}
struct Output;
impl Output {
    fn try_write_all(&mut self, bytes: &[u8]) -> Result<(), ErrorKind> {
        std::io::stdout().write_all(bytes).map_err(io_error)
    }
}
