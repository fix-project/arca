use anyhow::{Context, Result, bail, ensure};
use wasm_encoder::{
    CodeSection, Function, FunctionSection, ImportSection, Instruction, MemorySection, MemoryType,
    Module, RefType, SectionId, TableSection, TableType,
    reencode::{self, Reencode},
};
use wasmparser::{CompositeInnerType, FuncType, Parser, Payload, TypeRef, ValType};

const IMPORT_MODULE: &str = "__wasm_postprocessor";

struct Helper {
    type_index: u32,
    params: Vec<ValType>,
    results: Vec<ValType>,
    instruction: Instruction<'static>,
}

impl Helper {
    fn parse(
        name: &str,
        type_index: u32,
        memories: &[wasmparser::MemoryType],
        tables: &[wasmparser::TableType],
    ) -> Result<Self> {
        use Instruction::*;
        use ValType::I32;
        let memory = |text: &str| -> Result<u32> {
            let index: u32 = text.parse()?;
            let ty = memories
                .get(index as usize)
                .context("Memory index out of bounds")?;
            ensure!(
                !ty.memory64,
                "postprocessor operations require wasm32 Memories"
            );
            Ok(index)
        };
        let table = |text: &str| -> Result<(u32, ValType)> {
            let index: u32 = text.parse()?;
            let ty = tables
                .get(index as usize)
                .context("Table index out of bounds")?;
            ensure!(
                !ty.table64,
                "postprocessor operations require 32-bit Table indices"
            );
            Ok((index, ValType::Ref(ty.element_type)))
        };
        let parts: Vec<_> = name.split('_').collect();
        let (params, results, instruction) = match parts.as_slice() {
            ["memory", "copy", source, "to", destination] => (
                vec![I32, I32, I32],
                vec![],
                MemoryCopy {
                    src_mem: memory(source)?,
                    dst_mem: memory(destination)?,
                },
            ),
            ["table", "copy", source, "to", destination] => (
                vec![I32, I32, I32],
                vec![],
                TableCopy {
                    src_table: table(source)?.0,
                    dst_table: table(destination)?.0,
                },
            ),
            ["memory", operation, index] => {
                let index = memory(index)?;
                match *operation {
                    "size" => (vec![], vec![I32], MemorySize(index)),
                    "grow" => (vec![I32], vec![I32], MemoryGrow(index)),
                    "fill" => (vec![I32, I32, I32], vec![], MemoryFill(index)),
                    _ => bail!("unknown postprocessor import {name}"),
                }
            }
            ["table", operation, index] => {
                let (index, element) = table(index)?;
                match *operation {
                    "size" => (vec![], vec![I32], TableSize(index)),
                    "grow" => (vec![element, I32], vec![I32], TableGrow(index)),
                    "fill" => (vec![I32, element, I32], vec![], TableFill(index)),
                    "get" => (vec![I32], vec![element], TableGet(index)),
                    "set" => (vec![I32, element], vec![], TableSet(index)),
                    _ => bail!("unknown postprocessor import {name}"),
                }
            }
            _ => bail!("unknown postprocessor import {name}"),
        };
        Ok(Self {
            type_index,
            params,
            results,
            instruction,
        })
    }

    fn body(&self) -> Function {
        let mut function = Function::new([]);
        for index in 0..self.params.len() as u32 {
            function.instruction(&Instruction::LocalGet(index));
        }
        function.instruction(&self.instruction);
        function.instruction(&Instruction::End);
        function
    }
}

struct Postprocessor {
    helpers: Vec<Helper>,
    function_indices: Vec<u32>,
    added_memories: u32,
    added_tables: u32,
}

fn rank(section: SectionId) -> u8 {
    match section {
        SectionId::Custom => 0,
        SectionId::Type => 1,
        SectionId::Import => 2,
        SectionId::Function => 3,
        SectionId::Table => 4,
        SectionId::Memory => 5,
        SectionId::Tag => 6,
        SectionId::Global => 7,
        SectionId::Export => 8,
        SectionId::Start => 9,
        SectionId::Element => 10,
        SectionId::DataCount => 11,
        SectionId::Code => 12,
        SectionId::Data => 13,
    }
}

impl Postprocessor {
    fn add_memories(&self, section: &mut MemorySection) {
        for _ in 0..self.added_memories {
            section.memory(MemoryType {
                minimum: 0,
                maximum: None,
                memory64: false,
                shared: false,
                page_size_log2: None,
            });
        }
    }
    fn add_tables(&self, section: &mut TableSection) {
        for _ in 0..self.added_tables {
            section.table(TableType {
                element_type: RefType::EXTERNREF,
                minimum: 0,
                maximum: None,
                table64: false,
                shared: false,
            });
        }
    }
    fn add_functions(&self, section: &mut FunctionSection) {
        for helper in &self.helpers {
            section.function(helper.type_index);
        }
    }
    fn add_code(&self, section: &mut CodeSection) {
        for helper in &self.helpers {
            section.function(&helper.body());
        }
    }
}

impl Reencode for Postprocessor {
    type Error = std::io::Error;

    fn function_index(&mut self, index: u32) -> Result<u32, reencode::Error<Self::Error>> {
        self.function_indices
            .get(index as usize)
            .copied()
            .ok_or_else(|| {
                reencode::Error::UserError(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "function index out of bounds",
                ))
            })
    }
    fn parse_custom_name_subsection(
        &mut self,
        names: &mut wasm_encoder::NameSection,
        section: wasmparser::Name<'_>,
    ) -> Result<(), reencode::Error<Self::Error>> {
        match section {
            wasmparser::Name::Function(map) => {
                let mut entries = Vec::new();
                for entry in map {
                    let entry = entry?;
                    entries.push((self.function_index(entry.index)?, entry.name));
                }
                entries.sort_unstable_by_key(|entry| entry.0);
                let mut map = wasm_encoder::NameMap::new();
                for (index, name) in entries {
                    map.append(index, name);
                }
                names.functions(&map);
            }
            section @ (wasmparser::Name::Local(_) | wasmparser::Name::Label(_)) => {
                let locals = matches!(section, wasmparser::Name::Local(_));
                let map = match section {
                    wasmparser::Name::Local(map) | wasmparser::Name::Label(map) => map,
                    _ => unreachable!(),
                };
                let mut entries = Vec::new();
                for entry in map {
                    let entry = entry?;
                    entries.push((
                        self.function_index(entry.index)?,
                        reencode::utils::name_map(entry.names, Ok)?,
                    ));
                }
                entries.sort_unstable_by_key(|entry| entry.0);
                let mut map = wasm_encoder::IndirectNameMap::new();
                for (index, names) in entries {
                    map.append(index, &names);
                }
                if locals {
                    names.locals(&map);
                } else {
                    names.labels(&map);
                }
            }
            section => reencode::utils::parse_custom_name_subsection(self, names, section)?,
        }
        Ok(())
    }
    fn parse_import_section(
        &mut self,
        output: &mut ImportSection,
        section: wasmparser::ImportSectionReader<'_>,
    ) -> Result<(), reencode::Error<Self::Error>> {
        for import in section.into_imports() {
            let import = import?;
            if import.module != IMPORT_MODULE {
                self.parse_import(output, import)?;
            }
        }
        Ok(())
    }
    fn parse_function_section(
        &mut self,
        output: &mut FunctionSection,
        section: wasmparser::FunctionSectionReader<'_>,
    ) -> Result<(), reencode::Error<Self::Error>> {
        reencode::utils::parse_function_section(self, output, section)?;
        self.add_functions(output);
        Ok(())
    }
    fn parse_table_section(
        &mut self,
        output: &mut TableSection,
        section: wasmparser::TableSectionReader<'_>,
    ) -> Result<(), reencode::Error<Self::Error>> {
        reencode::utils::parse_table_section(self, output, section)?;
        self.add_tables(output);
        Ok(())
    }
    fn parse_memory_section(
        &mut self,
        output: &mut MemorySection,
        section: wasmparser::MemorySectionReader<'_>,
    ) -> Result<(), reencode::Error<Self::Error>> {
        reencode::utils::parse_memory_section(self, output, section)?;
        self.add_memories(output);
        Ok(())
    }
    fn parse_code_section(
        &mut self,
        output: &mut CodeSection,
        section: wasmparser::CodeSectionReader<'_>,
    ) -> Result<(), reencode::Error<Self::Error>> {
        reencode::utils::parse_code_section(self, output, section)?;
        self.add_code(output);
        Ok(())
    }
    fn intersperse_section_hook(
        &mut self,
        module: &mut Module,
        after: Option<SectionId>,
        before: Option<SectionId>,
    ) -> Result<(), reencode::Error<Self::Error>> {
        let lower = after.map(rank).unwrap_or(0);
        let upper = before.map(rank).unwrap_or(14);
        for id in [
            SectionId::Function,
            SectionId::Table,
            SectionId::Memory,
            SectionId::Code,
        ] {
            if !(lower < rank(id) && rank(id) < upper) {
                continue;
            }
            match id {
                SectionId::Function if !self.helpers.is_empty() => {
                    let mut section = FunctionSection::new();
                    self.add_functions(&mut section);
                    module.section(&section);
                }
                SectionId::Table if self.added_tables > 0 => {
                    let mut section = TableSection::new();
                    self.add_tables(&mut section);
                    module.section(&section);
                }
                SectionId::Memory if self.added_memories > 0 => {
                    let mut section = MemorySection::new();
                    self.add_memories(&mut section);
                    module.section(&section);
                }
                SectionId::Code if !self.helpers.is_empty() => {
                    let mut section = CodeSection::new();
                    self.add_code(&mut section);
                    module.section(&section);
                }
                _ => {}
            }
        }
        Ok(())
    }
}

pub fn process(
    wasm: &[u8],
    memory_count: Option<u32>,
    table_count: Option<u32>,
) -> Result<Vec<u8>> {
    let mut types = Vec::new();
    let mut memories = Vec::new();
    let mut tables = Vec::new();
    let mut imports = Vec::new();
    let mut defined_functions = 0;
    for payload in Parser::new(0).parse_all(wasm) {
        match payload? {
            Payload::TypeSection(section) => {
                for group in section {
                    for ty in group?.into_types() {
                        types.push(match ty.composite_type.inner {
                            CompositeInnerType::Func(ty) => Some(ty),
                            _ => None,
                        });
                    }
                }
            }
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import?;
                    ensure!(
                        import.module != IMPORT_MODULE || matches!(import.ty, TypeRef::Func(_)),
                        "postprocessor imports must be functions"
                    );
                    match import.ty {
                        TypeRef::Func(index) => imports.push((import.module, import.name, index)),
                        TypeRef::Memory(ty) => memories.push(ty),
                        TypeRef::Table(ty) => tables.push(ty),
                        _ => {}
                    }
                }
            }
            Payload::MemorySection(section) => {
                for memory in section {
                    memories.push(memory?);
                }
            }
            Payload::TableSection(section) => {
                for table in section {
                    tables.push(table?.ty);
                }
            }
            Payload::FunctionSection(section) => defined_functions = section.count(),
            _ => {}
        }
    }
    let target_memories = memory_count.unwrap_or(memories.len() as u32);
    let target_tables = table_count.unwrap_or(tables.len() as u32);
    ensure!(
        target_memories >= memories.len() as u32,
        "requested Memory count is smaller than existing count"
    );
    ensure!(
        target_tables >= tables.len() as u32,
        "requested Table count is smaller than existing count"
    );
    let mut processor = Postprocessor {
        helpers: Vec::new(),
        function_indices: Vec::new(),
        added_memories: target_memories - memories.len() as u32,
        added_tables: target_tables - tables.len() as u32,
    };
    memories.resize(
        target_memories as usize,
        wasmparser::MemoryType {
            initial: 0,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        },
    );
    tables.resize(
        target_tables as usize,
        wasmparser::TableType {
            element_type: wasmparser::RefType::EXTERNREF,
            initial: 0,
            maximum: None,
            table64: false,
            shared: false,
        },
    );
    let remaining_imports = imports
        .iter()
        .filter(|(module, _, _)| *module != IMPORT_MODULE)
        .count() as u32;
    let mut next_import = 0;
    for (module, name, type_index) in &imports {
        if *module == IMPORT_MODULE {
            let helper = Helper::parse(name, *type_index, &memories, &tables)
                .with_context(|| format!("invalid {IMPORT_MODULE} import {name}"))?;
            let ty: &FuncType = types
                .get(*type_index as usize)
                .and_then(Option::as_ref)
                .context("helper requires a function type")?;
            ensure!(
                ty.params() == helper.params && ty.results() == helper.results,
                "incorrect signature for {name}"
            );
            processor
                .function_indices
                .push(remaining_imports + defined_functions + processor.helpers.len() as u32);
            processor.helpers.push(helper);
        } else {
            processor.function_indices.push(next_import);
            next_import += 1;
        }
    }
    processor
        .function_indices
        .extend((0..defined_functions).map(|index| remaining_imports + index));
    let mut module = Module::new();
    processor.parse_core_module(&mut module, Parser::new(0), wasm)?;
    let output = module.finish();
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&output)
        .context("invalid postprocessed module")?;
    Ok(output)
}

#[cfg(test)]
#[path = "postprocessor_tests.rs"]
mod tests;
