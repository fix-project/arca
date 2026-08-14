use anyhow::Result;
use wasm_encoder::{
    MemorySection, MemoryType, Module, RawSection,
    reencode::{Reencode, RoundtripReencoder},
};
use wasmparser::{Parser, Payload};

pub fn process(wasm: &[u8]) -> Result<Vec<u8>> {
    let mut module = Module::new();
    for payload in Parser::new(0).parse_all(wasm) {
        match payload? {
            Payload::MemorySection(section) => {
                let mut memory_section = MemorySection::new();
                RoundtripReencoder.parse_memory_section(&mut memory_section, section)?;
                // Inject one hardcoded memory
                memory_section.memory(MemoryType {
                    minimum: 1,
                    maximum: None,
                    memory64: false,
                    shared: false,
                    page_size_log2: None,
                });
                module.section(&memory_section);
            }
            // Don't change other sections
            payload => {
                if let Some((id, range)) = payload.as_section() {
                    module.section(&RawSection {
                        id,
                        data: &wasm[range],
                    });
                }
            }
        }
    }

    Ok(module.finish())
}
