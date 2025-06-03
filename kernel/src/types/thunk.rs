pub mod syscall;

use core::ops::ControlFlow;

use elf::{endian::AnyEndian, segment::ProgramHeader, ElfBytes};
use syscall::handle_syscall;

use crate::{cpu::ExitReason, prelude::*};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Thunk {
    pub arca: Arca,
}

impl arca::RuntimeType for Thunk {
    type Runtime = Runtime;
}

impl arca::ValueType for Thunk {
    const DATATYPE: DataType = DataType::Thunk;
}

impl arca::Thunk for Thunk {
    fn run(self) -> arca::associated::Value<Self> {
        let mut cpu = crate::cpu::CPU.borrow_mut();
        self.run_on(&mut cpu)
    }

    fn read(
        self,
    ) -> (
        arca::associated::Blob<Self>,
        arca::associated::Table<Self>,
        arca::associated::Tree<Self>,
    ) {
        todo!()
    }
}

impl Thunk {
    pub fn new(arca: Arca) -> Thunk {
        Thunk { arca }
    }

    fn run_on(self, cpu: &mut Cpu) -> arca::associated::Value<Self> {
        let Thunk { arca } = self;
        let mut arca = arca.load(cpu);

        loop {
            let result = arca.run();
            match result {
                ExitReason::SystemCall => {}
                ExitReason::Interrupted(x) => {
                    if x == 0x20 {
                        continue;
                    }
                    panic!("exited with interrupt: {x:?}");
                }
                x => {
                    panic!(
                        "exited with exception: {x:x?} @ rip={:#x}",
                        arca.registers()[Register::RIP]
                    );
                }
            }
            if let ControlFlow::Break(result) = handle_syscall(&mut arca) {
                return result;
            }
        }
    }

    pub fn from_elf(elf: &[u8]) -> Thunk {
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

        let mut highest_addr = 0;

        for (i, segment) in segments.iter().enumerate() {
            log::debug!("processing segment: {:x?}", segment);
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
                        let mut unique_page = unsafe {
                            UniquePage::<Page4KB>::new_zeroed_in(BuddyAllocator).assume_init()
                        };
                        let page_start = page * 4096;
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
                                unique_page[(memi - page_start)..(copy_end - page_start)]
                                    .copy_from_slice(&data[filei..file_end]);
                                memi = page_end;
                                filei += copy_size;
                            } else {
                                memi = page_end;
                            }
                        }

                        if segment.p_flags & elf::abi::PF_W != 0 {
                            arca.mappings_mut().map(
                                page_start_memory + page_start,
                                arca::Entry::RWPage(CowPage::Unique(unique_page).into()),
                            );
                        } else {
                            arca.mappings_mut().map(
                                page_start_memory + page_start,
                                arca::Entry::ROPage(CowPage::Unique(unique_page).into()),
                            );
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

        let addr = highest_addr + 4096;
        let stack = unsafe { UniquePage::<Page4KB>::new_zeroed_in(BuddyAllocator).assume_init() };
        arca.mappings_mut()
            .map(addr, arca::Entry::RWPage(CowPage::Unique(stack).into()));
        arca.registers_mut()[Register::RSP] = addr as u64 + (1 << 12);
        Thunk { arca }
    }
}
