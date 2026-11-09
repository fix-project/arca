//! Fix expressions: Linux preprocessing followed by parsing inside Fix.

use fixpreprocessor as preprocessor;

use crate::{Client, Error, Value};
use fixapi::{Fix, ValueType};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

const PARSER: &[u8] = include_bytes!(env!("FIX_PARSER_MODULE"));

/// The label naming the compilation of the expression parser.
pub const PARSER_LABEL: &str = "fix-parser";

/// Adds the expression parser module and labels its compilation; returns the compilation.
pub fn install(client: &mut Client) -> Result<Value, Error> {
    let module = client.blob(PARSER)?;
    let compilation = client.compilation_request(&module, None)?;
    client.label(PARSER_LABEL, &compilation)?;
    Ok(compilation)
}

type Bindings = Vec<(String, String)>;

enum Kind {
    Handle(String),
    Label(String),
    File(PathBuf),
    Todo {
        reference: Option<String>,
        uris: Vec<String>,
        base: PathBuf,
    },
}

struct Scope {
    arguments: Vec<usize>,
    arity: usize,
}

struct Imports {
    repository: PathBuf,
    keys: BTreeMap<(preprocessor::Import, PathBuf), usize>,
    records: Vec<(usize, Kind)>,
    recorded: BTreeSet<usize>,
    overrides: Vec<Vec<String>>,
    stack: Vec<PathBuf>,
    scopes: Vec<Scope>,
    next: usize,
}

fn admit_reference(repository: &Path, name: &str) -> Result<(), String> {
    use fixhandle::{Handle, HandleBytesView, Name};
    let mut bytes = [0; 32];
    hex::decode_to_slice(name, &mut bytes).map_err(|error| format!("invalid handle: {error}"))?;
    let view = Handle::parse_ref(&bytes).map_err(|error| format!("invalid handle: {error:?}"))?;
    if !view.is_canonical() {
        return Err("expected a canonical handle".into());
    }
    if !matches!(view.view(), HandleBytesView::Ref(_)) {
        return Err("expected a Ref name".into());
    }
    if matches!(view.name(), Name::Literal(_)) {
        return Ok(());
    }
    bytes[31] &= 0xe0;
    let mut key = bytes;
    key[30..].fill(0xff);
    let target = PathBuf::from(format!("../data/{}", hex::encode(key)));
    let record = repository.join("objects").join(hex::encode({
        let mut reference = bytes;
        reference[31] |= 0x10;
        reference
    }));
    match std::os::unix::fs::symlink(&target, &record) {
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if fs::read_link(&record).map_err(|error| error.to_string())? != target {
                return Err("invalid object record".into());
            }
            Ok(())
        }
        result => result.map_err(|error| error.to_string()),
    }
}

impl Imports {
    fn allocate(&mut self) -> usize {
        self.next += 1;
        self.next - 1
    }

    fn key(&mut self, import: &preprocessor::Import, base: &Path) -> usize {
        let base = if matches!(import, preprocessor::Import::Binding(_)) {
            Path::new("")
        } else {
            base
        };
        let key = (import.clone(), base.to_owned());
        if let Some(index) = self.keys.get(&key) {
            return *index;
        }
        let index = self.allocate();
        self.keys.insert(key, index);
        index
    }

    fn expand(&mut self, source: &str, base: &Path) -> Result<String, String> {
        preprocessor::Preprocessor::new(source).preprocess_with(|import| {
            if let preprocessor::Import::Script {
                location,
                positionals,
                bindings,
            } = import
            {
                return self.script(&base.join(location), positionals, bindings, Some(base));
            }
            if let preprocessor::Import::Binding(name) = import {
                if name.chars().all(|ch| ch.is_ascii_digit()) {
                    let position: usize =
                        name.parse().map_err(|_| "invalid positional argument")?;
                    let scope = self
                        .scopes
                        .last_mut()
                        .ok_or("positional argument outside a script")?;
                    let index = *scope
                        .arguments
                        .get(position)
                        .ok_or_else(|| format!("missing positional argument ${name}"))?;
                    scope.arity = scope.arity.max(position + 1);
                    return Ok(format!("$import{index}"));
                }
                if !self.overrides.iter().any(|scope| scope.contains(name)) {
                    return Err(format!("missing named argument ${name}"));
                }
                let index = self.key(import, base);
                return Ok(format!("$import{index}"));
            }
            let index = self.key(import, base);
            if self.recorded.insert(index) {
                let kind = match import {
                    preprocessor::Import::Handle(name) => Kind::Handle(name.clone()),
                    preprocessor::Import::Label(name) => Kind::Label(name.clone()),
                    preprocessor::Import::File(path) => Kind::File(base.join(path)),
                    preprocessor::Import::Todo { reference, uris } => {
                        if uris
                            .iter()
                            .any(|uri| uri.is_empty() || uri.chars().any(char::is_control))
                        {
                            return Err(String::from("invalid Todo URI"));
                        }
                        if let Some(reference) = reference {
                            admit_reference(&self.repository, reference)?;
                        }
                        Kind::Todo {
                            reference: reference.clone(),
                            uris: uris.clone(),
                            base: base.to_owned(),
                        }
                    }
                    _ => unreachable!(),
                };
                self.records.push((index, kind));
            }
            Ok(format!("$import{index}"))
        })
    }

    fn source(
        &mut self,
        source: &str,
        base: &Path,
        positionals: &[String],
        bindings: &Bindings,
        bindings_base: &Path,
    ) -> Result<String, String> {
        let mut output = String::from("{\n");
        let mut arguments = Vec::new();
        for expression in positionals {
            let index = self.allocate();
            let expression = self.expand(expression, bindings_base)?;
            output.push_str(&format!("$import{index} = {expression}\n"));
            arguments.push(index);
        }
        for (name, expression) in bindings {
            if name.is_empty()
                || name.chars().all(|ch| ch.is_ascii_digit())
                || !name
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
            {
                return Err(format!("invalid binding name: {name}"));
            }
            let index = self.key(&preprocessor::Import::Binding(name.clone()), base);
            let expression = self.expand(expression, bindings_base)?;
            output.push_str(&format!("$import{index} = {expression}\n"));
        }
        self.overrides
            .push(bindings.iter().map(|(name, _)| name.clone()).collect());
        self.scopes.push(Scope {
            arguments,
            arity: 0,
        });
        let body = self.expand(source, base);
        let scope = self.scopes.pop().unwrap();
        self.overrides.pop();
        if body.is_ok() && scope.arguments.len() != scope.arity {
            return Err(format!(
                "expected {} positional arguments, got {}",
                scope.arity,
                scope.arguments.len()
            ));
        }
        output.push_str(&body?);
        output.push_str("\n}");
        Ok(output)
    }

    fn script(
        &mut self,
        path: &Path,
        positionals: &[String],
        bindings: &Bindings,
        bindings_base: Option<&Path>,
    ) -> Result<String, String> {
        let path =
            fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))?;
        if self.stack.contains(&path) {
            return Err(format!("recursive script import: {}", path.display()));
        }
        let source =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        self.stack.push(path.clone());
        let base = path.parent().unwrap().to_owned();
        let result = self.source(
            &source,
            &base,
            positionals,
            bindings,
            bindings_base.unwrap_or(&base),
        );
        self.stack.pop();
        result
    }
}

/// Resolves a URI relative to `base`; host paths and `file:` URIs become absolute `file:` URIs.
fn uri(location: &str, base: &Path) -> String {
    let scheme = location.split_once(':').filter(|(scheme, _)| {
        scheme.starts_with(|ch: char| ch.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || "+-.".contains(ch))
    });
    let path = match scheme {
        None => PathBuf::from(location),
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("file") => {
            let (authority, path) = match rest.strip_prefix("//") {
                Some(rest) => rest.split_at(rest.find('/').unwrap_or(rest.len())),
                None => ("", rest),
            };
            if !matches!(authority, "" | "localhost") || rest.contains(['?', '#']) {
                return location.into();
            }
            percent_decode(path)
        }
        Some(_) => return location.into(),
    };
    format!("file://{}", percent_encode(&base.join(path)))
}

fn percent_decode(text: &str) -> PathBuf {
    let bytes = text.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let Some(byte) = text
                .get(index + 1..index + 3)
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            output.push(byte);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(std::ffi::OsString::from_vec(output))
}

fn percent_encode(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let path = normalize(path);
    let mut output = String::new();
    for byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(byte) {
            output.push(*byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

fn normalize(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| {
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    normalized.pop();
                }
                std::path::Component::CurDir => {}
                component => normalized.push(component),
            }
        }
        normalized
    })
}

fn error(message: impl std::fmt::Display) -> Error {
    Error::Backend(message.to_string())
}

/// Parses expressions with named bindings and an evaluation policy for outer encodes.
pub struct Expressions<'a> {
    pub client: &'a mut Client,
    bindings: Bindings,
    parser: Option<Value>,
}

impl<'a> Expressions<'a> {
    pub fn new(client: &'a mut Client, bindings: Bindings) -> Self {
        Self {
            client,
            bindings,
            parser: None,
        }
    }

    fn parser(&mut self) -> Result<Value, Error> {
        if let Some(parser) = &self.parser {
            return Ok(parser.clone());
        }
        let parser = self.client.label_value(PARSER_LABEL)?;
        self.parser = Some(parser.clone());
        Ok(parser)
    }

    fn import(&mut self, kind: &Kind) -> Result<Value, Error> {
        match kind {
            Kind::Handle(name) => self.client.lookup(name),
            Kind::Label(name) => self.client.label_value(name),
            Kind::File(path) => self.client.file(path),
            Kind::Todo {
                reference,
                uris,
                base,
            } => {
                let uris: Vec<_> = uris.iter().map(|location| uri(location, base)).collect();
                if let Some(reference) = reference {
                    let value = self.client.lookup(reference)?;
                    self.client.todo_add(&value, &uris)?;
                    Ok(value)
                } else {
                    let value = self.client.todo_import(&uris[0])?;
                    let value = if self.client.value_type(&value)? == ValueType::Object {
                        self.client.reference(&value)?
                    } else {
                        value
                    };
                    let name = self.client.save(&value)?;
                    eprintln!("{} → {name}", uris[0]);
                    Ok(value)
                }
            }
        }
    }

    /// Parses `source` relative to `base` without evaluating it.
    pub fn parse(&mut self, source: &str, base: &Path) -> Result<Value, Error> {
        let repository = PathBuf::from(self.client.repository());
        let mut imports = Imports {
            repository,
            keys: Default::default(),
            records: Vec::new(),
            recorded: Default::default(),
            overrides: Vec::new(),
            stack: Vec::new(),
            scopes: Vec::new(),
            next: 0,
        };
        let bindings = self.bindings.clone();
        let expanded = imports
            .source(source, base, &[], &bindings, base)
            .map_err(error)?;
        let mut pairs = Vec::new();
        for (index, kind) in &imports.records {
            let name = self.client.blob(format!("import{index}").as_bytes())?;
            let value = self.import(kind)?;
            pairs.push(self.client.tree(&[name, value])?);
        }
        let environment = self.client.tree(&pairs)?;
        let text = self.client.blob(expanded.as_bytes())?;
        let limits = self.client.blob(b"")?;
        let parser = self.parser()?;
        let combination = self.client.tree(&[limits, parser, text, environment])?;
        let application = self.client.application(&combination)?;
        self.client.think(&application)
    }

    /// Parses `path` as a script invocation.
    pub fn parse_script(&mut self, path: &Path) -> Result<Value, Error> {
        let path = fs::canonicalize(path).map_err(|e| error(format!("{}: {e}", path.display())))?;
        let location = path
            .to_str()
            .ok_or_else(|| error("script path is not UTF-8"))?;
        let source = format!("@script({})", quote(location));
        self.parse(&source, &std::env::current_dir().map_err(error)?)
    }

    /// Evaluates an outer encode, leaving other values as constructed.
    pub fn realize(&mut self, value: Value) -> Result<Value, Error> {
        if self.client.value_type(&value)? == ValueType::Encode {
            self.client.eval(&value)
        } else {
            Ok(value)
        }
    }

    /// Parses and realizes each source, sharing one parse.
    pub fn arguments(&mut self, sources: &[String]) -> Result<Vec<Value>, Error> {
        let source = format!("({})", sources.join(" "));
        let tree = self.parse(&source, &std::env::current_dir().map_err(error)?)?;
        let count = self.client.len(&tree)? as usize;
        if count != sources.len() {
            return Err(error("expression argument count mismatch"));
        }
        (0..count)
            .map(|index| {
                let value = self.client.get(&tree, index)?;
                self.realize(value)
            })
            .collect()
    }
}

fn quote(text: &str) -> String {
    let mut quoted = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' | '\\' => {
                quoted.push('\\');
                quoted.push(ch);
            }
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    quoted
}
