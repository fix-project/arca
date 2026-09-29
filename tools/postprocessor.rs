use anyhow::Result;
use wasm_encoder::{
    MemorySection, MemoryType, Module, RawSection,
    reencode::{Reencode, RoundtripReencoder},
};
use wasmparser::{Parser, Payload};

const NUM_MEMORIES: u32 = macros::num_fixutils_memories!();

/* Adds `NUM_MEMORIES` memories to inputted `wasm` binary. This is required for binaries produced
 * from fix procedures that link with fixutils which provides multi-memory for interacting with
 * blobs through the fixshell. This extra postprocessing step is only needed because LLVM doesn't support
 * WebAssembly multi-memory and can be removed if/when multi-memory is supported.
 */
pub fn process(wasm: &[u8]) -> Result<Vec<u8>> {
    let mut module = Module::new();

    for payload in Parser::new(0).parse_all(wasm) {
        match payload? {
            Payload::MemorySection(section) => {
                let mut memories = MemorySection::new();
                RoundtripReencoder.parse_memory_section(&mut memories, section)?;
                for _ in 0..NUM_MEMORIES {
                    memories.memory(MemoryType {
                        minimum: 1,
                        maximum: None,
                        memory64: false,
                        shared: false,
                        page_size_log2: None,
                    });
                }
                module.section(&memories);
            }
            // Don't modify other sections
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
