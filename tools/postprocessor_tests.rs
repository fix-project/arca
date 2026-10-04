use super::process;
use wasm_encoder::{ImportSection, Module, TableSection, TableType, TypeSection, ValType};
use wasmparser::{ExternalKind, Operator, Parser, Payload};

const FIXTURE: &[u8] = include_bytes!(env!("POSTPROCESSOR_FIXTURE"));
const IMPORT_ONLY: &[u8] = include_bytes!(env!("POSTPROCESSOR_IMPORT_ONLY"));

fn invalid_fixture(helper_name: &str, params: &[ValType]) -> Vec<u8> {
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types.ty().function(params.iter().copied(), []);
    module.section(&types);
    let mut imports = ImportSection::new();
    imports.import(
        "__wasm_postprocessor",
        helper_name,
        wasm_encoder::EntityType::Function(0),
    );
    module.section(&imports);
    let mut tables = TableSection::new();
    tables.table(TableType {
        element_type: wasm_encoder::RefType::FUNCREF,
        minimum: 1,
        maximum: None,
        table64: false,
        shared: false,
    });
    module.section(&tables);
    module.finish()
}

#[test]
fn preserves_calls_exports_start_and_elements_when_replacing_imports() {
    let output = process(FIXTURE, Some(3), Some(4)).unwrap();
    let mut calls = Vec::new();
    let mut copies = Vec::new();
    let mut names = Vec::new();
    for payload in Parser::new(0).parse_all(&output) {
        match payload.unwrap() {
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import.unwrap();
                    names.push((import.module.to_owned(), import.name.to_owned()));
                }
            }
            Payload::MemorySection(section) => {
                assert_eq!(section.count(), 3);
                for memory in section {
                    assert_eq!(memory.unwrap().initial, 0);
                }
            }
            Payload::TableSection(section) => {
                let tables: Vec<_> = section.into_iter().map(|table| table.unwrap().ty).collect();
                assert_eq!(tables.len(), 4);
                assert_eq!(tables[0].element_type, wasmparser::RefType::FUNCREF);
                assert_eq!(tables[0].initial, 1);
                assert!(tables[1..].iter().all(|table| table.initial == 0
                    && table.element_type == wasmparser::RefType::EXTERNREF));
            }
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export.unwrap();
                    assert_eq!(export.kind, ExternalKind::Func);
                    assert_eq!(export.index, if export.name == "helper" { 2 } else { 1 });
                }
            }
            Payload::StartSection { func, .. } => assert_eq!(func, 1),
            Payload::ElementSection(section) => {
                for element in section {
                    let wasmparser::ElementItems::Functions(functions) = element.unwrap().items
                    else {
                        panic!()
                    };
                    assert_eq!(
                        functions
                            .into_iter()
                            .collect::<Result<Vec<_>, _>>()
                            .unwrap(),
                        [1]
                    );
                }
            }
            Payload::CodeSectionEntry(body) => {
                for operator in body.get_operators_reader().unwrap() {
                    match operator.unwrap() {
                        Operator::Call { function_index } => calls.push(function_index),
                        Operator::MemoryCopy { src_mem, dst_mem } => {
                            copies.push((src_mem, dst_mem))
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    assert_eq!(names, [("host".into(), "initialize".into())]);
    assert_eq!(calls, [0, 2]);
    assert_eq!(copies, [(0, 2)]);
}

#[test]
fn rejects_invalid_requests() {
    for (name, params, memories, tables, expected) in [
        (
            "memory_copy_0_to_2",
            vec![ValType::I32; 3],
            2,
            1,
            "invalid __wasm_postprocessor import",
        ),
        (
            "memory_copy_0_to_1",
            vec![ValType::I32; 2],
            2,
            1,
            "incorrect signature",
        ),
        (
            "memory_mvoe_0_to_1",
            vec![ValType::I32; 3],
            2,
            1,
            "invalid __wasm_postprocessor import",
        ),
        ("memory_size_0", vec![], 1, 0, "requested Table count"),
        (
            "table_copy_0_to_1",
            vec![ValType::I32; 3],
            1,
            2,
            "invalid postprocessed module",
        ),
    ] {
        let input = invalid_fixture(name, &params);
        let error = process(&input, Some(memories), Some(tables)).unwrap_err();
        assert!(error.to_string().contains(expected), "{name}: {error:#}");
    }
}

#[test]
fn inserts_resources_and_helper_sections_into_an_import_only_module() {
    let output = process(IMPORT_ONLY, Some(2), Some(2)).unwrap();
    let mut sizes = Vec::new();
    for payload in Parser::new(0).parse_all(&output) {
        if let Payload::CodeSectionEntry(body) = payload.unwrap() {
            for operator in body.get_operators_reader().unwrap() {
                if let Operator::TableSize { table } = operator.unwrap() {
                    sizes.push(table);
                }
            }
        }
    }
    assert_eq!(sizes, [1]);
}

#[test]
fn remaps_debug_names_in_index_order() {
    let mut input = FIXTURE.to_vec();
    let mut labels = wasm_encoder::IndirectNameMap::new();
    for index in 0..3 {
        labels.append(index, &wasm_encoder::NameMap::new());
    }
    let mut names = wasm_encoder::NameSection::new();
    names.labels(&labels);
    input.push(wasm_encoder::Section::id(&names));
    wasm_encoder::Encode::encode(&names, &mut input);
    let output = process(&input, Some(3), None).unwrap();
    let mut checked = 0;
    for payload in Parser::new(0).parse_all(&output) {
        if let Payload::CustomSection(section) = payload.unwrap()
            && let wasmparser::KnownCustom::Name(names) = section.as_known()
        {
            for name in names {
                match name.unwrap() {
                    wasmparser::Name::Function(map) => {
                        let names: Vec<_> = map
                            .into_iter()
                            .map(|entry| {
                                let entry = entry.unwrap();
                                (entry.index, entry.name)
                            })
                            .collect();
                        assert_eq!(names, [(0, "initialize"), (1, "apply"), (2, "helper")]);
                        checked += 1;
                    }
                    wasmparser::Name::Local(map) | wasmparser::Name::Label(map) => {
                        let indices: Vec<_> =
                            map.into_iter().map(|entry| entry.unwrap().index).collect();
                        assert_eq!(indices, [0, 1, 2]);
                        checked += 1;
                    }
                    _ => {}
                }
            }
        }
    }
    assert_eq!(checked, 3);
}
