use std::collections::HashMap;

use anyhow::Result;
use externref::processor::Processor;
use wasm_encoder::{
    CodeSection, Function, HeapType, Instruction, MemorySection, MemoryType, Module, RawSection,
    RefType, TableSection, TableType,
    reencode::{Reencode, RoundtripReencoder},
};
use wasmparser::{ExternalKind, Parser, Payload, TypeRef};

const PROGRAM_MEMORY: u32 = 0;
const FIX_MEMORY: u32 = 1;
const FIX_TABLE: u32 = 1;

pub fn instrument(wasm: &[u8]) -> Result<Vec<u8>> {
    let wasm = Processor::default().process_bytes(wasm)?;
    let mut module = Module::new();
    let mut function_imports = 0;
    let mut helpers = HashMap::new();
    let mut code = CodeSection::new();
    let mut num_functions = 0;

    for payload in Parser::new(0).parse_all(&wasm) {
        let payload = payload?;
        match &payload {
            Payload::ImportSection(section) => {
                for import in section.clone().into_imports().flatten() {
                    if matches!(import.ty, TypeRef::Func(_)) {
                        function_imports += 1;
                    }
                }
            }
            Payload::ExportSection(section) => {
                for export in section.clone() {
                    let export = export?;
                    if let Some(name) = export.name.strip_prefix("_instrument_")
                        && export.kind == ExternalKind::Func
                    {
                        helpers.insert(export.index, name);
                    }
                }
            }
            Payload::TableSection(section) => {
                let mut tables = TableSection::new();
                RoundtripReencoder.parse_table_section(&mut tables, section.clone())?;
                tables.table(TableType {
                    element_type: RefType::EXTERNREF,
                    table64: false,
                    minimum: 0,
                    maximum: None,
                    shared: false,
                });
                module.section(&tables);
                continue;
            }
            Payload::MemorySection(section) => {
                let mut memories = MemorySection::new();
                RoundtripReencoder.parse_memory_section(&mut memories, section.clone())?;
                memories.memory(MemoryType {
                    minimum: 0,
                    maximum: None,
                    memory64: false,
                    shared: false,
                    page_size_log2: None,
                });
                module.section(&memories);
                continue;
            }
            Payload::CodeSectionStart { count, .. } => {
                num_functions = *count;
                continue;
            }
            Payload::CodeSectionEntry(body) => {
                match helpers.get(&(function_imports + code.len())) {
                    Some(name) => code.function(&get_helper_body(name)),
                    None => code.raw(body.as_bytes()),
                };
                if code.len() == num_functions {
                    module.section(&code);
                }
                continue;
            }
            _ => {}
        }

        // don't modify other sections
        if let Some((id, range)) = payload.as_section() {
            module.section(&RawSection {
                id,
                data: &wasm[range],
            });
        }
    }

    Ok(module.finish())
}

fn get_helper_body(name: &str) -> Function {
    use Instruction::*;
    let instructions = match name {
        "memory_1_read" => vec![
            LocalGet(0),
            I32Const(0),
            LocalGet(1),
            MemoryCopy {
                src_mem: FIX_MEMORY,
                dst_mem: PROGRAM_MEMORY,
            },
        ],
        "memory_1_write" => vec![
            I32Const(0),
            LocalGet(0),
            LocalGet(1),
            MemoryCopy {
                src_mem: PROGRAM_MEMORY,
                dst_mem: FIX_MEMORY,
            },
        ],
        "memory_1_size" => vec![MemorySize(FIX_MEMORY)],
        "memory_1_grow" => vec![LocalGet(0), MemoryGrow(FIX_MEMORY)],
        "table_1_read" => vec![LocalGet(0), TableGet(FIX_TABLE)],
        "table_1_write" => vec![LocalGet(0), LocalGet(1), TableSet(FIX_TABLE)],
        "table_1_size" => vec![TableSize(FIX_TABLE)],
        "table_1_grow" => vec![RefNull(HeapType::EXTERN), LocalGet(0), TableGrow(FIX_TABLE)],
        _ => unreachable!("unknown instrumentation function: {name}"),
    };
    let mut function = Function::new([]);
    for instruction in instructions {
        function.instruction(&instruction);
    }
    function.instruction(&End);
    function
}
