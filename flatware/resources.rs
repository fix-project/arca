use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;
use wasmparser::{ExternalKind, FunctionBody, Operator, Parser, Payload, RefType, TypeRef};

fn operator_index(
    body: &FunctionBody<'_>,
    kind: &str,
    functions: &[FunctionBody<'_>],
    imported_functions: u32,
) -> Result<u32> {
    let mut result = None;
    for operator in body.get_operators_reader()? {
        let index = match (kind, operator?) {
            ("table", Operator::TableSize { table }) => Some(table),
            ("memory", Operator::MemorySize { mem }) => Some(mem),
            (_, Operator::Call { function_index }) => {
                let body = functions
                    .get(
                        function_index
                            .checked_sub(imported_functions)
                            .context("imported resource operation")?
                            as usize,
                    )
                    .context("missing resource operation")?;
                Some(operator_index(body, kind, functions, imported_functions)?)
            }
            _ => None,
        };
        if let Some(index) = index {
            ensure!(result.replace(index).is_none(), "ambiguous resource marker");
        }
    }
    result.context("missing resource marker instruction")
}

fn validate(wasm: &[u8]) -> Result<()> {
    let mut exports = BTreeMap::new();
    let mut functions = Vec::new();
    let mut imported_functions = 0;
    let mut tables = Vec::new();
    let mut memories = 0;
    for payload in Parser::new(0).parse_all(wasm) {
        match payload? {
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    match import?.ty {
                        TypeRef::Func(_) => imported_functions += 1,
                        TypeRef::Table(_) => {
                            anyhow::bail!("unresolved imported Tables are unsupported")
                        }
                        TypeRef::Memory(_) => {
                            anyhow::bail!("unresolved imported Memories are unsupported")
                        }
                        _ => {}
                    }
                }
            }
            Payload::TableSection(section) => {
                for table in section {
                    tables.push(table?.ty.element_type);
                }
            }
            Payload::MemorySection(section) => memories += section.count(),
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export?;
                    if export.kind == ExternalKind::Func {
                        exports.insert(export.name, export.index);
                    }
                }
            }
            Payload::CodeSectionEntry(body) => functions.push(body),
            _ => {}
        }
    }
    let marker = |name: &str, kind: &str| -> Result<u32> {
        let index = *exports
            .get(name)
            .with_context(|| format!("missing Flatware resource export {name}"))?;
        let body = functions
            .get(
                index
                    .checked_sub(imported_functions)
                    .context("imported resource marker")? as usize,
            )
            .context("missing resource marker body")?;
        operator_index(body, kind, &functions, imported_functions)
    };
    for (name, expected) in [("flatware_read_table_size", 0)] {
        let actual = marker(name, "table")?;
        ensure!(
            actual == expected && tables.get(actual as usize) == Some(&RefType::EXTERNREF),
            "invalid Flatware Table index or type: {name}"
        );
    }
    ensure!(
        marker("flatware_main_memory_size", "memory")? == 0,
        "invalid Flatware main Memory"
    );
    let scratch = marker("flatware_scratch_memory_size", "memory")?;
    ensure!(
        scratch == 1 && scratch < memories,
        "invalid Flatware scratch Memory"
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() == 3,
        "check_resources <input.wasm> <output.wasm>"
    );
    let wasm = std::fs::read(&args[1])?;
    validate(&wasm)?;
    std::fs::write(&args[2], wasm)?;
    Ok(())
}
