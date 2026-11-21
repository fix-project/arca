//! Fix expressions: Linux preprocessing followed by parsing inside Fix.

use fixpreprocessor as preprocessor;

use crate::shell::{Blob, Encode, Object, Tree};
use crate::{Error, Shell as Client};
use fix::ValueType;

pub type Value = fix::Value<Client>;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

type Bindings = Vec<(String, String)>;

const APPLICATION_MEMORY: u64 = 512 << 20;

enum Kind {
    Value(Value),
    Handle(String),
    Label(String),
    File(PathBuf),
    Compile(PathBuf),
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
    keys: BTreeMap<(preprocessor::Import, PathBuf), usize>,
    records: Vec<(usize, Kind)>,
    recorded: BTreeSet<usize>,
    overrides: Vec<Vec<String>>,
    stack: Vec<PathBuf>,
    scopes: Vec<Scope>,
    next: usize,
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
                    preprocessor::Import::Compile(path) => Kind::Compile(base.join(path)),
                    preprocessor::Import::Todo { reference, uris } => {
                        if uris
                            .iter()
                            .any(|uri| uri.is_empty() || uri.chars().any(char::is_control))
                        {
                            return Err(String::from("invalid Todo URI"));
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
        bindings: &[(String, String)],
        bindings_base: &Path,
    ) -> Result<String, String> {
        self.source_with(positionals, bindings, bindings_base, None, |imports| {
            imports.expand(source, base)
        })
    }

    fn source_with(
        &mut self,
        positionals: &[String],
        bindings: &[(String, String)],
        bindings_base: &Path,
        input: Option<Value>,
        body: impl FnOnce(&mut Self) -> Result<String, String>,
    ) -> Result<String, String> {
        let mut output = String::from("{\n");
        let mut arguments = Vec::new();
        if let Some(input) = input {
            let index = self.allocate();
            self.records.push((index, Kind::Value(input)));
            arguments.push(index);
        }
        let arity = arguments.len();
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
            let index = self.key(&preprocessor::Import::Binding(name.clone()), bindings_base);
            let expression = self.expand(expression, bindings_base)?;
            output.push_str(&format!("$import{index} = {expression}\n"));
        }
        self.overrides
            .push(bindings.iter().map(|(name, _)| name.clone()).collect());
        self.scopes.push(Scope { arguments, arity });
        let body = body(self);
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
        bindings: &[(String, String)],
        bindings_base: Option<&Path>,
    ) -> Result<String, String> {
        let path =
            fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))?;
        if self.stack.contains(&path) {
            return Err(format!("recursive script import: {}", path.display()));
        }
        let source =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        self.script_source(&path, &source, positionals, bindings, bindings_base)
    }

    fn script_source(
        &mut self,
        path: &Path,
        source: &str,
        positionals: &[String],
        bindings: &[(String, String)],
        bindings_base: Option<&Path>,
    ) -> Result<String, String> {
        self.in_script(path, |imports, base| {
            imports.source(
                source,
                base,
                positionals,
                bindings,
                bindings_base.unwrap_or(base),
            )
        })
    }

    fn in_script<T>(
        &mut self,
        path: &Path,
        body: impl FnOnce(&mut Self, &Path) -> Result<T, String>,
    ) -> Result<T, String> {
        self.stack.push(path.to_owned());
        let result = body(self, path.parent().unwrap());
        self.stack.pop();
        result
    }

    fn root_source(
        &mut self,
        source: &str,
        base: &Path,
        path: Option<&Path>,
        arguments: &[String],
    ) -> Result<String, String> {
        match path {
            Some(path) => self.script_source(path, source, arguments, &[], Some(base)),
            None if arguments.is_empty() => self.expand(source, base),
            None => self.source(source, base, arguments, &[], base),
        }
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
        let parser = self.client.lookup(&self.client.options().parser.clone())?;
        self.parser = Some(parser.clone());
        Ok(parser)
    }

    fn import(&mut self, kind: &Kind) -> Result<Value, Error> {
        match kind {
            Kind::Value(value) => Ok(value.clone()),
            Kind::Handle(name) => self.client.lookup(name),
            Kind::Label(name) => self.client.label_value(name),
            Kind::File(path) => self.client.file(path),
            Kind::Compile(path) => {
                let module = self.client.file(path)?;
                self.client.compilation(&module, Default::default())
            }
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
                    let value = if value.value_type() == ValueType::Object {
                        Object::try_from(value)?.reference().into_value()
                    } else {
                        value
                    };
                    let name = self.client.save(&value)?;
                    common::progress::println(format!("{} → {name}", uris[0]));
                    Ok(value)
                }
            }
        }
    }

    /// Parses `source` relative to `base` without evaluating it.
    pub fn parse(&mut self, source: &str, base: &Path) -> Result<Value, Error> {
        self.parse_with(source, base, |imports| imports.expand(source, base))
    }

    fn parse_with(
        &mut self,
        source: &str,
        base: &Path,
        expand: impl FnOnce(&mut Imports) -> Result<String, String>,
    ) -> Result<Value, Error> {
        let mut imports = Imports {
            keys: Default::default(),
            records: Vec::new(),
            recorded: Default::default(),
            overrides: Vec::new(),
            stack: Vec::new(),
            scopes: Vec::new(),
            next: 0,
        };
        let expanded = imports
            .source_with(&[], &self.bindings, base, None, expand)
            .map_err(error)?;
        let mut pairs = Vec::new();
        for (index, kind) in &imports.records {
            let name = Blob::new(format!("import{index}").as_bytes());
            let value = self.import(kind)?;
            pairs.push(Tree::new(&[name.into(), value]).into_value());
        }
        let environment = Tree::new(&pairs);
        let text = Blob::new(expanded.as_bytes());
        let limits = Blob::new(&APPLICATION_MEMORY.to_le_bytes());
        let parser = self.parser()?;
        let combination = Tree::new(&[limits.into(), parser, text.into(), environment.into()]);
        let result = self.client.call(&combination.into_value())?;
        let parser = self.client.options().parser;
        let envelope = Tree::try_from(result.clone()).map_err(|cause| {
            error(format!(
                "parser {parser} returned a non-Tree response for {source:?}: {cause:?}"
            ))
        })?;
        if envelope.len() == 1 {
            return envelope
                .get(0)?
                .ok_or_else(|| error("parser success response has no value"));
        }
        if envelope.len() == 2 {
            let fields = envelope.into_iter().collect::<Result<Vec<_>, _>>()?;
            if let (Ok(kind), Ok(detail)) = (Blob::try_from(&fields[0]), Blob::try_from(&fields[1]))
            {
                let kind = kind.to_vec()?;
                let detail = detail.to_vec()?;
                match kind.as_slice() {
                    b"Rust.panic" | b"Fix.error" => {
                        return Err(error(format!(
                            "parser {parser} failed for {source:?}: {}",
                            String::from_utf8_lossy(&detail)
                        )));
                    }
                    b"Wasm.trap" if detail.len() == 4 => {
                        return Err(error(format!(
                            "parser {parser} trapped for {source:?}: Wasm trap code {}",
                            u32::from_le_bytes(detail.try_into().unwrap())
                        )));
                    }
                    _ => (),
                }
            }
        }
        let result = self.client.save(&result)?;
        Err(error(format!(
            "parser {parser} returned malformed response {result} for {source:?}; expected a one-element success Tree"
        )))
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

    pub fn run_script(
        &mut self,
        path: Option<&Path>,
        limit: Option<u64>,
        arguments: &[String],
    ) -> Result<Value, Error> {
        let cwd = std::env::current_dir().map_err(error)?;
        let path = path
            .map(|path| {
                fs::canonicalize(path).map_err(|e| error(format!("{}: {e}", path.display())))
            })
            .transpose()?;
        let bytes = match &path {
            Some(path) => fs::read(path).map_err(|e| error(format!("{}: {e}", path.display())))?,
            None => {
                let mut bytes = Vec::new();
                std::io::Read::read_to_end(&mut std::io::stdin().lock(), &mut bytes)
                    .map_err(error)?;
                bytes
            }
        };
        let decode = |bytes| {
            std::str::from_utf8(bytes).map_err(|e| match &path {
                Some(path) => error(format!("{}: {e}", path.display())),
                None => error(e),
            })
        };
        let Some(header) = bytes.strip_prefix(b"#!") else {
            let source = decode(&bytes)?;
            return self
                .parse_with(source, &cwd, |imports| {
                    imports.root_source(source, &cwd, path.as_deref(), arguments)
                })?
                .eval();
        };
        let line = header
            .split(|byte| *byte == b'\n')
            .next()
            .unwrap_or_default();
        let source = decode(line)?.trim();
        if source.is_empty() {
            return Err(error("empty shebang expression"));
        }
        let limit = limit.unwrap_or(APPLICATION_MEMORY);
        let bindings = [("limit".to_owned(), format!("{limit}u64"))];
        let input = Blob::new(&bytes).into_value();
        self.parse_with(source, &cwd, |imports| {
            imports.source_with(
                arguments,
                &bindings,
                &cwd,
                Some(input),
                |imports| match &path {
                    Some(path) => {
                        imports.in_script(path, |imports, base| imports.expand(source, base))
                    }
                    None => imports.expand(source, &cwd),
                },
            )
        })?
        .eval()
    }

    /// Evaluates an outer encode, leaving other values as constructed.
    pub fn realize(&mut self, value: Value) -> Result<Value, Error> {
        if value.value_type() == ValueType::Encode {
            Encode::try_from(value)?.eval()
        } else {
            Ok(value)
        }
    }

    /// Parses and realizes each source, sharing one parse.
    pub fn arguments(&mut self, sources: &[String]) -> Result<Vec<Value>, Error> {
        let source = format!("({})", sources.join(" "));
        let tree = self.parse(&source, &std::env::current_dir().map_err(error)?)?;
        let tree = Tree::try_from(tree)?;
        let count = tree.len();
        if count != sources.len() {
            return Err(error(format!(
                "parser {} returned {count} values for {} expression arguments {sources:?}; expected one result per argument",
                self.client.options().parser,
                sources.len()
            )));
        }
        tree.into_iter().map(|value| self.realize(value?)).collect()
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
