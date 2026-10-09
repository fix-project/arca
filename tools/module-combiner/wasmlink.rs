// SPDX-License-Identifier: Apache-2.0
// Copyright 2016 WebAssembly Community Group participants

use anyhow::{Context, Result, bail, ensure};
use std::collections::{HashMap, HashSet};
use wasm_encoder::{
    CodeSection, DataCountSection, DataSection, ElementSection, ExportSection, Function,
    FunctionSection, GlobalSection, ImportSection, Instruction, MemorySection, Module,
    StartSection, TableSection, TagSection, TypeSection,
    reencode::{Error, Reencode},
};
use wasmparser::{
    Encoding, Export, ExternalKind, Import, Parser, Payload, TypeRef, Validator, WasmFeatures,
};

const UNRESOLVED: u32 = u32::MAX;
const KINDS: [ExternalKind; 5] = [
    ExternalKind::Func,
    ExternalKind::Table,
    ExternalKind::Memory,
    ExternalKind::Global,
    ExternalKind::Tag,
];

#[derive(Default)]
struct IndexSpaces<T> {
    functions: T,
    tables: T,
    memories: T,
    globals: T,
    tags: T,
}

impl<T> IndexSpaces<T> {
    fn get(&self, kind: ExternalKind) -> &T {
        match kind {
            ExternalKind::Func | ExternalKind::FuncExact => &self.functions,
            ExternalKind::Table => &self.tables,
            ExternalKind::Memory => &self.memories,
            ExternalKind::Global => &self.globals,
            ExternalKind::Tag => &self.tags,
        }
    }

    fn get_mut(&mut self, kind: ExternalKind) -> &mut T {
        match kind {
            ExternalKind::Func | ExternalKind::FuncExact => &mut self.functions,
            ExternalKind::Table => &mut self.tables,
            ExternalKind::Memory => &mut self.memories,
            ExternalKind::Global => &mut self.globals,
            ExternalKind::Tag => &mut self.tags,
        }
    }
}

fn import_kind(ty: TypeRef) -> ExternalKind {
    match ty {
        TypeRef::Func(_) | TypeRef::FuncExact(_) => ExternalKind::Func,
        TypeRef::Table(_) => ExternalKind::Table,
        TypeRef::Memory(_) => ExternalKind::Memory,
        TypeRef::Global(_) => ExternalKind::Global,
        TypeRef::Tag(_) => ExternalKind::Tag,
    }
}

#[derive(Default)]
struct Indices {
    entities: IndexSpaces<Vec<u32>>,
    types: u32,
    elements: u32,
    data: u32,
}

impl Reencode for Indices {
    type Error = std::convert::Infallible;

    fn function_index(&mut self, index: u32) -> Result<u32, Error<Self::Error>> {
        Ok(self.entities.functions[index as usize])
    }
    fn table_index(&mut self, index: u32) -> Result<u32, Error<Self::Error>> {
        Ok(self.entities.tables[index as usize])
    }
    fn memory_index(&mut self, index: u32) -> Result<u32, Error<Self::Error>> {
        Ok(self.entities.memories[index as usize])
    }
    fn global_index(&mut self, index: u32) -> Result<u32, Error<Self::Error>> {
        Ok(self.entities.globals[index as usize])
    }
    fn tag_index(&mut self, index: u32) -> Result<u32, Error<Self::Error>> {
        Ok(self.entities.tags[index as usize])
    }
    fn type_index(&mut self, index: u32) -> Result<u32, Error<Self::Error>> {
        Ok(self.types + index)
    }
    fn element_index(&mut self, index: u32) -> Result<u32, Error<Self::Error>> {
        Ok(self.elements + index)
    }
    fn data_index(&mut self, index: u32) -> Result<u32, Error<Self::Error>> {
        Ok(self.data + index)
    }
}

struct Input<'a> {
    name: &'a str,
    bytes: &'a [u8],
    imports: Vec<(Import<'a>, u32)>,
    exports: HashMap<&'a str, Export<'a>>,
    imported: IndexSpaces<u32>,
    indices: Indices,
    types: u32,
    elements: u32,
    data: u32,
}

impl<'a> Input<'a> {
    fn read(name: &'a str, bytes: &'a [u8]) -> Result<Self> {
        Validator::new_with_features(WasmFeatures::all()).validate_all(bytes)?;
        let mut input = Self {
            name,
            bytes,
            imports: Vec::new(),
            exports: HashMap::new(),
            imported: IndexSpaces::default(),
            indices: Indices::default(),
            types: 0,
            elements: 0,
            data: 0,
        };
        for payload in Parser::new(0).parse_all(bytes) {
            match payload? {
                Payload::Version { encoding, .. } => {
                    ensure!(encoding == Encoding::Module, "expected a core Wasm module")
                }
                Payload::TypeSection(section) => {
                    for group in section {
                        input.types += group?.types().len() as u32;
                    }
                }
                Payload::ImportSection(section) => {
                    for import in section.into_imports() {
                        let import = import?;
                        let kind = import_kind(import.ty);
                        let indices = input.indices.entities.get_mut(kind);
                        input.imports.push((import, indices.len() as u32));
                        indices.push(UNRESOLVED);
                        *input.imported.get_mut(kind) += 1;
                    }
                }
                Payload::FunctionSection(section) => input.indices.entities.functions.resize(
                    input.imported.functions as usize + section.count() as usize,
                    UNRESOLVED,
                ),
                Payload::TableSection(section) => input.indices.entities.tables.resize(
                    input.imported.tables as usize + section.count() as usize,
                    UNRESOLVED,
                ),
                Payload::MemorySection(section) => input.indices.entities.memories.resize(
                    input.imported.memories as usize + section.count() as usize,
                    UNRESOLVED,
                ),
                Payload::GlobalSection(section) => input.indices.entities.globals.resize(
                    input.imported.globals as usize + section.count() as usize,
                    UNRESOLVED,
                ),
                Payload::TagSection(section) => input.indices.entities.tags.resize(
                    input.imported.tags as usize + section.count() as usize,
                    UNRESOLVED,
                ),
                Payload::ExportSection(section) => {
                    for export in section {
                        let export = export?;
                        input.exports.insert(export.name, export);
                    }
                }
                Payload::ElementSection(section) => input.elements = section.count(),
                Payload::DataSection(section) => input.data = section.count(),
                _ => {}
            }
        }
        Ok(input)
    }
}

pub fn merge(
    inputs: &[(&str, &[u8])],
    rename_exports: bool,
    reject_resource_imports: bool,
) -> Result<Vec<u8>> {
    ensure!(!inputs.is_empty(), "no input modules");
    let mut modules = inputs
        .iter()
        .map(|(name, bytes)| Input::read(name, bytes).with_context(|| format!("module {name}")))
        .collect::<Result<Vec<_>>>()?;
    let mut names = HashMap::new();
    for (index, module) in modules.iter().enumerate() {
        ensure!(
            names.insert(module.name, index).is_none(),
            "duplicate module name: {}",
            module.name
        );
    }
    let mut totals = IndexSpaces::<u32>::default();
    let mut aliases = Vec::new();
    for (source, module) in modules.iter_mut().enumerate() {
        for (import, index) in &module.imports {
            let kind = import_kind(import.ty);
            if let Some(&target) = names.get(import.module) {
                aliases.push((source, kind, *index, target, import.name));
            } else {
                ensure!(
                    !reject_resource_imports
                        || !matches!(kind, ExternalKind::Memory | ExternalKind::Table),
                    "unresolved resource import {}::{}",
                    import.module,
                    import.name
                );
                module.indices.entities.get_mut(kind)[*index as usize] = *totals.get(kind);
                *totals.get_mut(kind) += 1;
            }
        }
    }
    let mut type_count = 0;
    let mut element_count = 0;
    let mut data_count = 0;
    for module in &mut modules {
        module.indices.types = type_count;
        module.indices.elements = element_count;
        module.indices.data = data_count;
        type_count += module.types;
        element_count += module.elements;
        data_count += module.data;
        for kind in KINDS {
            for index in
                &mut module.indices.entities.get_mut(kind)[*module.imported.get(kind) as usize..]
            {
                *index = *totals.get(kind);
                *totals.get_mut(kind) += 1;
            }
        }
    }
    while !aliases.is_empty() {
        let previous = aliases.len();
        let mut remaining = Vec::new();
        for (source, kind, index, target, name) in aliases {
            let export = modules[target]
                .exports
                .get(name)
                .with_context(|| format!("missing export {}::{name}", modules[target].name))?;
            ensure!(
                kind == export.kind
                    || matches!(
                        (kind, export.kind),
                        (ExternalKind::Func, ExternalKind::FuncExact)
                    ),
                "import/export kind mismatch: {name}"
            );
            let value = modules[target].indices.entities.get(kind)[export.index as usize];
            if value == UNRESOLVED {
                remaining.push((source, kind, index, target, name));
            } else {
                modules[source].indices.entities.get_mut(kind)[index as usize] = value;
            }
        }
        ensure!(remaining.len() < previous, "cyclic import aliases");
        aliases = remaining;
    }
    let mut types = TypeSection::new();
    let mut imports = ImportSection::new();
    let mut functions = FunctionSection::new();
    let mut tables = TableSection::new();
    let mut memories = MemorySection::new();
    let mut tags = TagSection::new();
    let mut globals = GlobalSection::new();
    let mut exports = ExportSection::new();
    let mut elements = ElementSection::new();
    let mut code = CodeSection::new();
    let mut data = DataSection::new();
    let mut starts = Vec::new();
    let mut export_names = HashSet::new();
    for module in &mut modules {
        let indices = &mut module.indices;
        for payload in Parser::new(0).parse_all(module.bytes) {
            match payload? {
                Payload::TypeSection(section) => indices.parse_type_section(&mut types, section)?,
                Payload::ImportSection(section) => {
                    for import in section.into_imports() {
                        let import = import?;
                        if !names.contains_key(import.module) {
                            imports.import(
                                import.module,
                                import.name,
                                indices.entity_type(import.ty)?,
                            );
                        }
                    }
                }
                Payload::FunctionSection(section) => {
                    indices.parse_function_section(&mut functions, section)?
                }
                Payload::TableSection(section) => {
                    indices.parse_table_section(&mut tables, section)?
                }
                Payload::MemorySection(section) => {
                    indices.parse_memory_section(&mut memories, section)?
                }
                Payload::TagSection(section) => indices.parse_tag_section(&mut tags, section)?,
                Payload::GlobalSection(section) => {
                    indices.parse_global_section(&mut globals, section)?
                }
                Payload::ExportSection(section) => {
                    for export in section {
                        let export = export?;
                        let mut name = export.name.to_owned();
                        if !export_names.insert(name.clone()) {
                            ensure!(rename_exports, "duplicate export: {name}");
                            let prefix = format!("{}.{name}", module.name);
                            name = prefix.clone();
                            let mut suffix = 1;
                            while !export_names.insert(name.clone()) {
                                name = format!("{prefix}.{suffix}");
                                suffix += 1;
                            }
                        }
                        exports.export(
                            &name,
                            indices.export_kind(export.kind)?,
                            indices.external_index(export.kind, export.index)?,
                        );
                    }
                }
                Payload::StartSection { func, .. } => starts.push(indices.function_index(func)?),
                Payload::ElementSection(section) => {
                    indices.parse_element_section(&mut elements, section)?
                }
                Payload::CodeSectionEntry(body) => indices.parse_function_body(&mut code, body)?,
                Payload::DataSection(section) => indices.parse_data_section(&mut data, section)?,
                Payload::Version { .. }
                | Payload::CodeSectionStart { .. }
                | Payload::DataCountSection { .. }
                | Payload::CustomSection(_)
                | Payload::End(_) => {}
                _ => bail!("unsupported module section"),
            }
        }
    }
    let start = match starts.as_slice() {
        [] => None,
        [start] => Some(*start),
        _ => {
            types.ty().function([], []);
            functions.function(type_count);
            let mut initializer = Function::new([]);
            for start in starts {
                initializer.instruction(&Instruction::Call(start));
            }
            initializer.instruction(&Instruction::End);
            code.function(&initializer);
            Some(totals.functions)
        }
    };
    let mut output = Module::new();
    if !types.is_empty() {
        output.section(&types);
    }
    if !imports.is_empty() {
        output.section(&imports);
    }
    if !functions.is_empty() {
        output.section(&functions);
    }
    if !tables.is_empty() {
        output.section(&tables);
    }
    if !memories.is_empty() {
        output.section(&memories);
    }
    if !tags.is_empty() {
        output.section(&tags);
    }
    if !globals.is_empty() {
        output.section(&globals);
    }
    if !exports.is_empty() {
        output.section(&exports);
    }
    if let Some(function_index) = start {
        output.section(&StartSection { function_index });
    }
    if !elements.is_empty() {
        output.section(&elements);
    }
    if data_count > 0 {
        output.section(&DataCountSection { count: data_count });
    }
    if !code.is_empty() {
        output.section(&code);
    }
    if !data.is_empty() {
        output.section(&data);
    }
    let output = output.finish();
    Validator::new_with_features(WasmFeatures::all()).validate_all(&output)?;
    Ok(output)
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut inputs = Vec::new();
    let mut output = None;
    let mut rename = false;
    let mut reject_resource_imports = false;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-o" => output = Some(args.next().context("missing output filename")?),
            "--rename-export-conflicts" => rename = true,
            "--no-resource-imports" => reject_resource_imports = true,
            "--all-features" | "--disable-gc" => {}
            _ if argument.starts_with('-') => bail!("unknown option: {argument}"),
            _ => {
                let name = args.next().context("expected INPUT NAME pairs")?;
                inputs.push((
                    name,
                    std::fs::read(&argument).with_context(|| format!("reading {argument}"))?,
                ));
            }
        }
    }
    let output =
        output.context("usage: wasm-merge INPUT NAME ... -o OUTPUT [--rename-export-conflicts]")?;
    let inputs: Vec<_> = inputs
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect();
    std::fs::write(output, merge(&inputs, rename, reject_resource_imports)?)?;
    Ok(())
}
