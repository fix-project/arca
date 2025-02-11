use alloc::{boxed::Box, string::String, vec, vec::Vec};
use elf::{endian::AnyEndian, segment::ProgramHeader, ElfBytes};

use crate::{
    prelude::*,
    types::pagetable::{AnyEntry, Entry},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Thunk {
    pub arca: Arca,
}

impl Thunk {
    pub fn new<T: Into<Value>>(f: Lambda, x: T) -> Thunk {
        let mut arca = f.arca;
        let idx = f.idx;
        arca.descriptors_mut()[idx] = x.into();
        Thunk { arca }
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

                    if segment.p_flags & elf::abi::PF_W != 0 {
                        arca.mappings_mut()
                            .map(page_start, AnyEntry::Entry4KB(Entry::UniquePage(page)));
                    } else {
                        arca.mappings_mut().map(
                            page_start,
                            AnyEntry::Entry4KB(Entry::SharedPage(page.into())),
                        );
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

        let addr = (1 << 21) + (3 << 12);
        let stack =
            unsafe { UniquePage::<Page4KB>::new_zeroed_in(&PHYSICAL_ALLOCATOR).assume_init() };
        arca.mappings_mut()
            .map(addr, AnyEntry::Entry4KB(Entry::UniquePage(stack)));
        arca.registers_mut()[Register::RSP] = addr as u64 + (1 << 12);
        Thunk { arca }
    }

    pub fn run(self) -> Value {
        let mut cpu = CPU.borrow_mut();
        let Thunk { arca } = self;
        let mut arca = arca.load(&mut cpu);
        loop {
            let result = arca.run();
            if result.code != 256 {
                arca.unload();
                log::debug!("exited with exception: {result:?}");
                let tree = if result.code == 14 {
                    log::info!(
                        "user page fault @ {:p}",
                        crate::registers::read_cr2() as *const ()
                    );
                    vec![
                        Value::Atom("page fault".into()),
                        Value::Blob(crate::registers::read_cr2().to_ne_bytes().into()),
                        Value::Blob(result.error.to_ne_bytes().into()),
                    ]
                } else {
                    vec![
                        Value::Atom("exception".into()),
                        Value::Blob(result.code.to_ne_bytes().into()),
                        Value::Blob(result.error.to_ne_bytes().into()),
                    ]
                };
                return Value::Error(Value::Tree(tree.into()).into());
            }
            let regs = arca.registers();
            let num = regs[Register::RDI];
            let args = &[
                regs[Register::RSI],
                regs[Register::RDX],
                regs[Register::R10],
                regs[Register::R8],
                regs[Register::R9],
            ];
            log::debug!("exited with syscall: {num:#x?}({args:?})");
            let result = &mut [0, 0];
            match num {
                defs::syscall::RESIZE => {
                    let len = args[0] as usize;
                    arca.descriptors_mut().resize(len, Value::Null);
                    result[0] = 0;
                }
                defs::syscall::NULL => {
                    let idx = args[0] as usize;
                    if let Some(x) = arca.descriptors_mut().get_mut(idx) {
                        *x = Value::Null;
                        result[0] = 0;
                    } else {
                        result[0] = defs::error::BAD_INDEX;
                    }
                }
                defs::syscall::EXIT => {
                    let idx = args[0] as usize;
                    if let Some(x) = arca.descriptors_mut().get_mut(idx) {
                        let mut val = Value::Null;
                        core::mem::swap(x, &mut val);
                        arca.unload();
                        return val;
                    };
                    log::warn!("exit failed with invalid index");
                    result[0] = defs::error::BAD_INDEX;
                }
                defs::syscall::READ => 'read: {
                    let idx = args[0] as usize;
                    let Some(val) = arca.descriptors_mut().get(idx) else {
                        result[0] = defs::error::BAD_INDEX;
                        break 'read;
                    };
                    match val {
                        Value::Blob(blob) => {
                            let ptr = args[1] as usize;
                            let len = args[2] as usize;
                            let len = core::cmp::min(len, blob.len());
                            unsafe {
                                let success = crate::vm::copy_kernel_to_user(ptr, &blob[..len]);

                                if success {
                                    result[0] = len as i64;
                                } else {
                                    result[0] = defs::error::BAD_ARGUMENT
                                }
                            }
                        }
                        Value::Tree(tree) => {
                            let tree = tree.clone();
                            let ptr = args[1] as usize;
                            let len = args[2] as usize;
                            let len = core::cmp::min(len, tree.len());
                            let mut buffer = vec![0u8; len * core::mem::size_of::<u64>()];
                            unsafe {
                                let success = crate::vm::copy_user_to_kernel(&mut buffer, ptr);
                                if !success {
                                    result[0] = defs::error::BAD_ARGUMENT;
                                    break 'read;
                                }
                            }
                            let indices = buffer.chunks(core::mem::size_of::<u64>()).map(|x| {
                                let bytes: [u8; core::mem::size_of::<u64>()] = x.try_into().ok()?;
                                Some(u64::from_ne_bytes(bytes) as usize)
                            });
                            for (x, i) in tree.iter().zip(indices) {
                                let Some(i) = i else {
                                    result[0] = defs::error::BAD_INDEX;
                                    break 'read;
                                };
                                arca.descriptors_mut()[i] = x.clone();
                            }
                            result[0] = len as i64;
                        }
                        _ => {
                            log::warn!("READ called with invalid type");
                            result[0] = defs::error::BAD_TYPE;
                        }
                    }
                }
                defs::syscall::CREATE_BLOB => 'create: {
                    let idx = args[0] as usize;
                    if idx >= arca.descriptors().len() {
                        result[0] = defs::error::BAD_INDEX;
                        break 'create;
                    }
                    let ptr = args[1] as usize;
                    let len = args[2] as usize;
                    unsafe {
                        let mut buffer = Box::new_uninit_slice(len).assume_init();
                        let success = crate::vm::copy_user_to_kernel(&mut buffer, ptr);
                        if !success {
                            result[0] = defs::error::BAD_ARGUMENT;
                            break 'create;
                        }
                        arca.descriptors_mut()[idx] = Value::Blob(buffer.into());
                        result[0] = len as i64;
                    }
                }
                defs::syscall::CONTINUATION => {
                    let idx = args[0] as usize;
                    if idx >= arca.descriptors().len() {
                        result[0] = defs::error::BAD_INDEX;
                    } else {
                        let unloaded = arca.unload();
                        let mut copy = unloaded.clone();
                        copy.registers_mut()[Register::RAX] = defs::error::CONTINUED as u64;
                        arca = unloaded.load(&mut cpu);
                        arca.descriptors_mut()[idx] = Value::Thunk(Thunk { arca: copy });
                        result[0] = 0;
                    }
                }
                defs::syscall::PROMPT => {
                    let idx = args[0] as usize;
                    if idx >= arca.descriptors().len() {
                        result[0] = defs::error::BAD_INDEX;
                    } else {
                        let mut arca = arca.unload();
                        arca.registers_mut()[Register::RAX] = 0;
                        return Value::Lambda(Lambda { arca, idx });
                    }
                }
                defs::syscall::SHOW => 'show: {
                    let ptr = args[0] as usize;
                    let len = args[1] as usize;
                    let msg = unsafe {
                        let mut buffer = Box::new_uninit_slice(len).assume_init();
                        let success = crate::vm::copy_user_to_kernel(&mut buffer, ptr);
                        if !success {
                            break 'show;
                        }
                        String::from_utf8_lossy(&buffer).into_owned()
                    };
                    let idx = args[2] as usize;
                    if idx >= arca.descriptors().len() {
                        break 'show;
                    }
                    let val = &arca.descriptors()[idx];
                    log::info!("user message - \"{msg}\": {val:?}");
                    result[0] = 0;
                }
                defs::syscall::LOG => 'log: {
                    let ptr = args[0] as usize;
                    let len = args[1] as usize;
                    let msg = unsafe {
                        let mut buffer = Box::new_uninit_slice(len).assume_init();
                        let success = crate::vm::copy_user_to_kernel(&mut buffer, ptr);
                        if !success {
                            break 'log;
                        }
                        String::from_utf8_lossy(&buffer).into_owned()
                    };
                    log::info!("user message - \"{msg}\"");
                    result[0] = 0;
                }
                _ => {
                    log::error!("invalid syscall {num:#x}");
                    result[0] = defs::error::BAD_SYSCALL;
                }
            }
            let regs = arca.registers_mut();
            regs[Register::RAX] = result[0] as u64;
            regs[Register::RDX] = result[1] as u64;
        }
    }
}
