#![no_main]
#![no_std]
#![feature(allocator_api)]

extern crate alloc;

extern crate kernel;

use alloc::vec::Vec;
use common::refcnt::RefCnt;
use elf::{endian::AnyEndian, segment::ProgramHeader, ElfBytes};
use kernel::{
    allocator::PHYSICAL_ALLOCATOR,
    cpu::{Register, CPU},
    page::{Page2MB, Page4KB, UniquePage},
    paging::{
        PageTable as _, PageTable1GB, PageTable2MB, PageTable512GB, PageTableEntry as _,
        UnmappedPage,
    },
    shutdown,
    types::Arca,
};

const TRAP_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_trap"));
const IDENTITY_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_identity"));

fn load(elf: &[u8]) -> Arca {
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
                    UnmappedPage::Table(table) => table,
                    UnmappedPage::None => RefCnt::from(PageTable512GB::new()),
                    _ => panic!("invalid mapping (L3) @ 0x0"),
                };
                let mut pd = match RefCnt::make_mut(&mut pdpt)[i3].unmap() {
                    UnmappedPage::Table(table) => table,
                    UnmappedPage::None => RefCnt::from(PageTable1GB::new()),
                    _ => panic!("invalid mapping (L2) @ {i3:#x}"),
                };
                let mut pt = match RefCnt::make_mut(&mut pd)[i2].unmap() {
                    UnmappedPage::Table(table) => table,
                    UnmappedPage::None => RefCnt::from(PageTable2MB::new()),
                    _ => panic!("invalid mapping (L1) @ {i2:#x}"),
                };
                RefCnt::make_mut(&mut pt)[i1].map_unique(page);
                RefCnt::make_mut(&mut pd)[i2].chain(pt);
                RefCnt::make_mut(&mut pdpt)[i3].chain(pd);
                arca.mappings_mut().chain(pdpt);
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

    let addr = 0x0;
    let stack = unsafe { UniquePage::<Page2MB>::new_zeroed_in(&PHYSICAL_ALLOCATOR).assume_init() };
    let i1 = (addr >> 12) & 0x1ff;
    assert_eq!(i1, 0);
    let i2 = (addr >> 21) & 0x1ff;
    let i3 = (addr >> 30) & 0x1ff;

    let pdpt = arca.mappings_mut().unmap();
    let mut pdpt = match pdpt {
        UnmappedPage::Table(table) => table,
        UnmappedPage::None => RefCnt::from(PageTable512GB::new()),
        _ => panic!("invalid mapping (L3) @ 0x0"),
    };
    let mut pd = match RefCnt::make_mut(&mut pdpt)[i3].unmap() {
        UnmappedPage::Table(table) => table,
        UnmappedPage::None => RefCnt::from(PageTable1GB::new()),
        _ => panic!("invalid mapping (L2) @ {i3:#x}"),
    };
    RefCnt::make_mut(&mut pd)[i2].map_unique(stack);
    RefCnt::make_mut(&mut pdpt)[i3].chain(pd);
    arca.mappings_mut().chain(pdpt);
    arca.registers_mut()[Register::RSP] = addr as u64 + (1 << 21);
    arca
}

#[no_mangle]
#[inline(never)]
extern "C" fn kmain() -> ! {
    log::info!("kmain");
    let mut cpu = CPU.borrow_mut();

    let arca = load(TRAP_ELF);
    let mut arca = arca.load(&mut cpu);
    log::info!("running trap program");
    let result = arca.run();
    assert_eq!(result.code, 3);

    let arca = load(IDENTITY_ELF);
    let mut arca = arca.load(&mut cpu);
    log::info!("running identity program");
    let result = arca.run();
    assert_eq!(result.code, 256);
    assert_eq!(arca.registers()[Register::RDI], 0);
    log::info!("done");
    shutdown();
}
