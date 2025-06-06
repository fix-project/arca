use arca::prelude::*;
use arca::Entry;
use elf::{endian::AnyEndian, segment::ProgramHeader, ElfBytes};

extern crate alloc;
use alloc::vec::Vec;

pub fn load_elf<R: arca::Runtime>(runtime: &R, elf: &[u8]) -> R::Thunk {
    log::debug!("loading: {} byte ELF file", elf.len());
    let elf = ElfBytes::<AnyEndian>::minimal_parse(elf).expect("could not parse elf");
    let start_address = elf.ehdr.e_entry;
    let segments: Vec<ProgramHeader> = elf
        .segments()
        .expect("could not find ELF segments")
        .iter()
        .collect();

    assert_eq!(elf.ehdr.e_type, elf::abi::ET_EXEC);

    let mut registers = [0; 20];
    registers[16] = start_address;

    let mut highest_addr = 0;

    let mut table = runtime.create_table(0);

    for (i, segment) in segments.iter().enumerate() {
        log::info!("processing segment: {:x?}", segment);
        match segment.p_type {
            elf::abi::PT_NOTE => {
                // ignore for now
            }
            elf::abi::PT_LOAD => {
                let start = segment.p_vaddr as usize;
                let page_start_memory = (start / 4096) * 4096;
                let offset = start - page_start_memory;
                let filesz = segment.p_filesz as usize;
                let memsz = segment.p_memsz as usize;

                let mut pages = (offset + memsz) / 4096;
                if (offset + memsz % 4096) > 0 {
                    pages += 1;
                }

                let mut memi = offset;
                let mut filei = 0;
                let data = elf.segment_data(segment).expect("could not find segment");
                for page in 0..pages {
                    let page_start = page * 4096;
                    let mut unique_page = table
                        .unmap(page_start_memory + page_start)
                        .and_then(|entry| match entry {
                            Entry::ROPage(page) | Entry::RWPage(page) => Some(page),
                            _ => None,
                        })
                        .unwrap_or_else(|| runtime.create_page(1 << 12));
                    assert!(memi >= page_start);
                    let page_end = page_start + 4096;
                    if memi >= page_start && memi < page_end {
                        if filei < filesz {
                            let mem_left = page_end - memi;
                            let file_left = filesz - filei;

                            let copy_size = core::cmp::min(mem_left, file_left);
                            let file_end = filei + copy_size;

                            let copy_end = memi + copy_size;

                            assert!(copy_end - memi <= 4096);
                            unique_page.write(memi - page_start, &data[filei..file_end]);
                            memi = page_end;
                            filei += copy_size;
                        } else {
                            memi = page_end;
                        }
                    }

                    if segment.p_flags & elf::abi::PF_W != 0 {
                        table
                            .map(page_start_memory + page_start, Entry::RWPage(unique_page))
                            .unwrap();
                    } else {
                        table
                            .map(page_start_memory + page_start, Entry::ROPage(unique_page))
                            .unwrap();
                    }
                    highest_addr = core::cmp::max(highest_addr, page_start_memory + page_start);
                }
            }
            elf::abi::PT_PHDR => {
                // program header
            }
            0x60000000..0x70000000 => {
                // os-specific
            }
            0x70000000..0x80000000 => {
                // arch-specific
            }
            x => unimplemented!("{i} - segment type {x:#x}"),
        }
    }

    let bytes: Vec<u8> = registers
        .into_iter()
        .flat_map(|x| x.to_ne_bytes())
        .collect();

    let registers = runtime.create_blob(&bytes);
    let descriptors = runtime.create_tree(0);
    runtime.create_thunk(registers, table, descriptors)
}
