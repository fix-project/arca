use anyhow::Result;
use wasm_encoder::{
    MemorySection, MemoryType, Module, RawSection,
    reencode::{Reencode, RoundtripReencoder},
};
use wasmparser::{Parser, Payload};

pub fn process(wasm: &[u8]) -> Result<Vec<u8>> {
    let mut module = Module::new();
    let mut num_memories: u32 = 0;
    let mut memory_section: Option<MemorySection> = None;
    // Sections that come after memory_section
    let mut trailing_sections = Vec::new();

    for payload in Parser::new(0).parse_all(wasm) {
        match payload? {
            Payload::MemorySection(section) => {
                let mut memories = MemorySection::new();
                RoundtripReencoder.parse_memory_section(&mut memories, section)?;
                memory_section = Some(memories);
            }
            Payload::CustomSection(section) if section.name() == "wasm_num_memories" => {
                num_memories = u32::from_le_bytes(section.data().try_into()?);
            }
            // Don't change other sections
            payload => {
                if let Some((id, range)) = payload.as_section() {
                    let section = RawSection {
                        id,
                        data: &wasm[range],
                    };
                    if memory_section.is_some() {
                        trailing_sections.push(section);
                    } else {
                        module.section(&section);
                    }
                }
            }
        }
    }

    if let Some(mut mem_section) = memory_section {
        for _ in 0..num_memories {
            mem_section.memory(MemoryType {
                minimum: 1,
                maximum: None,
                memory64: false,
                shared: false,
                page_size_log2: None,
            });
        }
        module.section(&mem_section);
    }
    for section in trailing_sections {
        module.section(&section);
    }

    Ok(module.finish())
}
