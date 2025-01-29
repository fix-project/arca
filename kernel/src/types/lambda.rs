use alloc::{boxed::Box, vec::Vec};
use elf::{endian::AnyEndian, segment::ProgramHeader, ElfBytes};

use crate::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lambda {
    pub arca: Arca,
}

impl Lambda {
    pub fn new(arca: Arca) -> Lambda {
        Lambda { arca }
    }

    pub fn from_elf(elf: &[u8]) -> Lambda {
        log::debug!("loading: {} byte ELF file", elf.len());
        let elf = ElfBytes::<AnyEndian>::minimal_parse(elf).expect("could not parse elf");
        let start_address = elf.ehdr.e_entry;
        let segments: Vec<ProgramHeader> = elf
            .segments()
            .expect("could not find ELF segments")
            .iter()
            .collect();

        assert_eq!(elf.ehdr.e_type, elf::abi::ET_EXEC);

        let mut arca = Arca::new();
        arca.registers_mut()[Register::RIP] = start_address;

        for (i, segment) in segments.iter().enumerate() {
            log::debug!("processing segment: {:?}", segment);
            match segment.p_type {
                elf::abi::PT_LOAD => {
                    let start = segment.p_vaddr as usize;
                    let page_start = (start / 4096) * 4096;
                    let offset = start - page_start;
                    let filesz = segment.p_filesz as usize;
                    let memsz = segment.p_memsz as usize;
                    assert!(offset + memsz <= 4096);
                    let data = elf.segment_data(segment).expect("could not find segment");
                    let mut page = unsafe {
                        UniquePage::<Page4KB>::new_zeroed_in(&PHYSICAL_ALLOCATOR).assume_init()
                    };
                    page[offset..offset + filesz].copy_from_slice(data);
                    assert_eq!(page_start & 0xfff, 0);
                    let i1 = (page_start >> 12) & 0x1ff;
                    let i2 = (page_start >> 21) & 0x1ff;
                    let i3 = (page_start >> 30) & 0x1ff;

                    let pdpt = arca.mappings_mut().unmap();
                    let mut pdpt = match pdpt {
                        UnmappedPage::SharedTable(table) => SharedPage::into_unique(table),
                        UnmappedPage::UniqueTable(table) => table,
                        UnmappedPage::None => PageTable512GB::new(),
                        _ => panic!("invalid mapping (L3) @ 0x0"),
                    };
                    let mut pd = match pdpt[i3].unmap() {
                        UnmappedPage::SharedTable(table) => SharedPage::into_unique(table),
                        UnmappedPage::UniqueTable(table) => table,
                        UnmappedPage::None => PageTable1GB::new(),
                        _ => panic!("invalid mapping (L2) @ {i3:#x}"),
                    };
                    let mut pt = match pd[i2].unmap() {
                        UnmappedPage::SharedTable(table) => SharedPage::into_unique(table),
                        UnmappedPage::UniqueTable(table) => table,
                        UnmappedPage::None => PageTable2MB::new(),
                        _ => panic!("invalid mapping (L1) @ {i2:#x}"),
                    };
                    if segment.p_flags & elf::abi::PF_W != 0 {
                        pt[i1].map_unique(page);
                    } else {
                        pt[i1].map_shared(page.into());
                    }
                    pd[i2].chain_unique(pt);
                    pdpt[i3].chain_unique(pd);
                    arca.mappings_mut().chain_unique(pdpt);
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

        let addr = 1 << 30;
        let stack =
            unsafe { UniquePage::<Page4KB>::new_zeroed_in(&PHYSICAL_ALLOCATOR).assume_init() };
        let i1 = (addr >> 12) & 0x1ff;
        assert_eq!(i1, 0);
        let i2 = (addr >> 21) & 0x1ff;
        let i3 = (addr >> 30) & 0x1ff;

        let pdpt = arca.mappings_mut().unmap();
        let mut pdpt = match pdpt {
            UnmappedPage::SharedTable(table) => SharedPage::into_unique(table),
            UnmappedPage::UniqueTable(table) => table,
            UnmappedPage::None => PageTable512GB::new(),
            _ => panic!("invalid mapping (L3) @ 0x0"),
        };
        let mut pd = match pdpt[i3].unmap() {
            UnmappedPage::SharedTable(table) => SharedPage::into_unique(table),
            UnmappedPage::UniqueTable(table) => table,
            UnmappedPage::None => PageTable1GB::new(),
            _ => panic!("invalid mapping (L2) @ {i3:#x}"),
        };
        let mut pt = match pd[i2].unmap() {
            UnmappedPage::SharedTable(table) => SharedPage::into_unique(table),
            UnmappedPage::UniqueTable(table) => table,
            UnmappedPage::None => PageTable2MB::new(),
            _ => panic!("invalid mapping (L1) @ {i2:#x}"),
        };
        pt[i1].map_unique(stack);
        pd[i2].chain_unique(pt);
        pdpt[i3].chain_unique(pd);
        arca.mappings_mut().chain_unique(pdpt);
        arca.registers_mut()[Register::RSP] = addr as u64 + (1 << 12);
        Lambda { arca }
    }

    pub fn apply<T: Into<Box<Value>>>(self, x: T) -> Thunk {
        Thunk::new(self, x)
    }
}
