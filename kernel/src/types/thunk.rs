use core::time::Duration;

use defs::error;
use elf::{endian::AnyEndian, segment::ProgramHeader, ElfBytes};
use time::OffsetDateTime;

use crate::{
    cpu::ExitReason,
    kvmclock,
    prelude::*,
    types::pagetable::{AnyEntry, Entry},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Thunk {
    pub arca: Arca,
}

#[derive(Debug)]
pub struct LoadedThunk<'a> {
    pub arca: LoadedArca<'a>,
}

impl Thunk {
    pub fn new<T: Into<Value>>(f: Lambda, x: T) -> Thunk {
        let mut arca = f.arca;
        let idx = f.idx;
        let x: Value = x.into();
        arca.descriptors_mut()[idx] = x;
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

        let mut highest_addr = 0;

        for (i, segment) in segments.iter().enumerate() {
            log::debug!("processing segment: {:x?}", segment);
            match segment.p_type {
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
                            UniquePage::<Page4KB>::new_zeroed_in(&PHYSICAL_ALLOCATOR).assume_init()
                        };
                        let page_start = page * 4096;
                        assert!(memi >= page_start);
                        let page_end = page_start + 4096;
                        if memi >= page_start && memi < page_end && filei < filesz {
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
                        }

                        if segment.p_flags & elf::abi::PF_W != 0 {
                            arca.mappings_mut().map(
                                page_start_memory + page_start,
                                AnyEntry::Entry4KB(Entry::UniquePage(unique_page)),
                            );
                        } else {
                            arca.mappings_mut().map(
                                page_start_memory + page_start,
                                AnyEntry::Entry4KB(Entry::SharedPage(unique_page.into())),
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
        let stack =
            unsafe { UniquePage::<Page4KB>::new_zeroed_in(&PHYSICAL_ALLOCATOR).assume_init() };
        arca.mappings_mut()
            .map(addr, AnyEntry::Entry4KB(Entry::UniquePage(stack)));
        arca.registers_mut()[Register::RSP] = addr as u64 + (1 << 12);
        Thunk { arca }
    }

    pub fn load(self, cpu: &mut Cpu) -> LoadedThunk<'_> {
        let arca = self.arca.load(cpu);
        LoadedThunk { arca }
    }

    pub fn run(self, cpu: &mut Cpu) -> Value {
        let loaded = self.load(cpu);
        let result = loaded.run();
        result.into()
    }
}

impl<'a> LoadedThunk<'a> {
    pub fn new<T: Into<Value>>(f: LoadedLambda<'_>, x: T) -> LoadedThunk<'_> {
        let mut arca = f.arca;
        let idx = f.idx;
        arca.descriptors_mut()[idx] = x.into();
        LoadedThunk { arca }
    }

    pub fn unload(self) -> Thunk {
        Thunk {
            arca: self.arca.unload(),
        }
    }

    pub fn run(self) -> LoadedValue<'a> {
        self.run_for(Duration::from_secs(1))
    }

    pub fn run_for(self, timeout: Duration) -> LoadedValue<'a> {
        let start = kvmclock::now();
        let end = start + timeout;
        self.run_until(end)
    }

    pub fn run_until(self, alarm: OffsetDateTime) -> LoadedValue<'a> {
        let LoadedThunk { mut arca } = self;
        loop {
            let result = arca.run();
            match result {
                ExitReason::SystemCall => {}
                ExitReason::Interrupted(x) => {
                    if x == 0x20 {
                        let now = kvmclock::now();
                        if now < alarm {
                            log::debug!("program was interrupted, but time has not expired");
                            continue;
                        } else {
                            return LoadedValue::Thunk(LoadedThunk { arca });
                        }
                    }
                    log::info!("exited with interrupt: {x:?}");
                    let tree = vec![
                        LoadedValue::Unloaded(Value::Atom("interrupt".into())),
                        LoadedValue::Unloaded(Value::Blob(x.to_ne_bytes().into())),
                        LoadedValue::Thunk(LoadedThunk { arca }),
                    ];
                    return LoadedValue::Error(LoadedValue::Tree(tree).into());
                }
                x => {
                    log::warn!(
                        "exited with exception: {x:x?} @ rip={:#x}",
                        arca.registers()[Register::RIP]
                    );
                    let tree = vec![
                        LoadedValue::Unloaded(Value::Atom("exception".into())),
                        LoadedValue::Unloaded(x.into()),
                        LoadedValue::Thunk(LoadedThunk { arca }),
                    ];
                    return LoadedValue::Error(LoadedValue::Tree(tree).into());
                }
            }
            let regs = arca.registers();
            let num = regs[Register::RDI];
            let args = [
                regs[Register::RSI],
                regs[Register::RDX],
                regs[Register::R10],
                regs[Register::R8],
                regs[Register::R9],
            ];
            log::debug!("exited with syscall: {num:#x?}({args:?})");
            let result = match num {
                defs::syscall::NOP => Ok(0),
                defs::syscall::NULL => sys_null(args, &mut arca),
                defs::syscall::RESIZE => sys_resize(args, &mut arca),

                defs::syscall::EXIT => match sys_exit(args, arca) {
                    Ok(result) => return result,
                    Err((a, e)) => {
                        arca = a;
                        Err(e)
                    }
                },
                defs::syscall::LEN => sys_len(args, &mut arca),
                defs::syscall::READ => sys_read(args, &mut arca),
                defs::syscall::TYPE => sys_type(args, &mut arca),

                defs::syscall::CREATE_BLOB => sys_create_blob(args, &mut arca),
                defs::syscall::CREATE_TREE => sys_create_tree(args, &mut arca),

                defs::syscall::CONTINUATION => sys_continuation(args, &mut arca),
                defs::syscall::APPLY => sys_apply(args, &mut arca),
                defs::syscall::PROMPT => match sys_prompt(args, arca) {
                    Ok(result) => return result,
                    Err((a, e)) => {
                        arca = a;
                        Err(e)
                    }
                },
                defs::syscall::PERFORM => match sys_perform(args, arca) {
                    Ok(result) => return result,
                    Err((a, e)) => {
                        arca = a;
                        Err(e)
                    }
                },
                defs::syscall::TAILCALL => sys_tailcall(args, &mut arca),

                defs::syscall::SHOW => sys_show(args, &mut arca),
                defs::syscall::LOG => sys_log(args, &mut arca),

                _ => {
                    log::error!("invalid syscall {num:#x}");
                    Err(error::BAD_SYSCALL)
                }
            };
            let regs = arca.registers_mut();
            regs[Register::RAX] = match result {
                Ok(x) => x as u64,
                Err(e) => -(e as i64) as u64,
            };
        }
    }
}

// type ExitSyscall = fn([u64; 5], LoadedArca) -> Result<LoadedValue, (LoadedArca, u32)>;
// type Syscall = fn([u64; 5], &mut LoadedArca) -> Result<u32, u32>;

fn sys_null(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let idx = args[0] as usize;
    if let Some(x) = arca.descriptors_mut().get_mut(idx) {
        *x = Value::Null;
        Ok(0)
    } else {
        Err(error::BAD_INDEX)
    }
}

fn sys_resize(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let len = args[0] as usize;
    arca.descriptors_mut().resize(len, Value::Null);
    Ok(0)
}

#[allow(clippy::result_large_err)]
fn sys_exit(args: [u64; 5], mut arca: LoadedArca) -> Result<LoadedValue, (LoadedArca, u32)> {
    let idx = args[0] as usize;
    if let Some(x) = arca.descriptors_mut().get_mut(idx) {
        let x = core::mem::take(x);
        arca.unload();
        Ok(LoadedValue::Unloaded(x))
    } else {
        log::warn!("exit failed with invalid index");
        Err((arca, error::BAD_INDEX))
    }
}

fn sys_len(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let idx = args[0] as usize;
    let Some(val) = arca.descriptors().get(idx) else {
        return Err(error::BAD_INDEX);
    };
    let ptr = args[1] as usize;
    let bytes: [u8; 8] = match val {
        Value::Blob(blob) => blob.len().to_ne_bytes(),
        Value::Tree(tree) => tree.len().to_ne_bytes(),
        _ => return Err(error::BAD_TYPE),
    };

    unsafe {
        let success = crate::vm::copy_kernel_to_user(ptr, &bytes);
        if success {
            Ok(0)
        } else {
            Err(error::BAD_ARGUMENT)
        }
    }
}

fn sys_read(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let idx = args[0] as usize;
    let Some(val) = arca.descriptors_mut().get_mut(idx) else {
        return Err(error::BAD_INDEX);
    };
    match val {
        Value::Blob(blob) => {
            let ptr = args[1] as usize;
            let len = args[2] as usize;
            let len = core::cmp::min(len, blob.len());
            unsafe {
                let success = crate::vm::copy_kernel_to_user(ptr, &blob[..len]);

                if success {
                    Ok(len.try_into().expect("length was too long"))
                } else {
                    Err(error::BAD_ARGUMENT)
                }
            }
        }
        Value::Tree(_) => {
            let value = core::mem::take(val);
            let Value::Tree(mut tree) = value else {
                panic!();
            };
            let tree = Arc::make_mut(&mut tree);
            let ptr = args[1] as usize;
            let len = args[2] as usize;
            let len = core::cmp::min(len, tree.len());
            let mut buffer = vec![0u8; len * core::mem::size_of::<u64>()];
            unsafe {
                let success = crate::vm::copy_user_to_kernel(&mut buffer, ptr);
                if !success {
                    return Err(error::BAD_ARGUMENT);
                }
            }
            let indices = buffer.chunks(core::mem::size_of::<u64>()).map(|x| {
                let bytes: [u8; core::mem::size_of::<u64>()] = x.try_into().ok()?;
                Some(u64::from_ne_bytes(bytes) as usize)
            });
            for (x, i) in tree.iter_mut().zip(indices) {
                let Some(i) = i else {
                    return Err(error::BAD_INDEX);
                };
                let x = core::mem::take(x);
                arca.descriptors_mut()[i] = x;
            }
            Ok(len.try_into().expect("length was too long"))
        }
        _ => {
            log::warn!("READ called with invalid type: {val:?}");
            Err(error::BAD_TYPE)
        }
    }
}

fn sys_type(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let idx = args[0] as usize;
    let Some(val) = arca.descriptors().get(idx) else {
        return Err(error::BAD_INDEX);
    };
    match val {
        Value::Null => Ok(defs::types::NULL),
        Value::Error(_) => Ok(defs::types::ERROR),
        Value::Atom(_) => Ok(defs::types::ATOM),
        Value::Blob(_) => Ok(defs::types::BLOB),
        Value::Tree(_) => Ok(defs::types::TREE),
        Value::Page(_) => Ok(defs::types::PAGE),
        Value::PageTable(_) => Ok(defs::types::PAGETABLE),
        Value::Lambda(_) => Ok(defs::types::LAMBDA),
        Value::Thunk(_) => Ok(defs::types::THUNK),
    }
    .map(|x| x.try_into().expect("type was too large"))
}

fn sys_create_blob(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let idx = args[0] as usize;
    if idx >= arca.descriptors().len() {
        return Err(error::BAD_INDEX);
    }
    let ptr = args[1] as usize;
    let len = args[2] as usize;
    unsafe {
        let mut buffer = Box::new_uninit_slice(len).assume_init();
        let success = crate::vm::copy_user_to_kernel(&mut buffer, ptr);
        if !success {
            return Err(error::BAD_ARGUMENT);
        }
        arca.descriptors_mut()[idx] = Value::Blob(buffer.into());
        Ok(len.try_into().expect("length was too long"))
    }
}

fn sys_create_tree(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let idx = args[0] as usize;
    if idx >= arca.descriptors().len() {
        return Err(error::BAD_INDEX);
    }
    let ptr = args[1] as usize;
    let len = args[2] as usize;
    let mut buffer = vec![0u8; len * core::mem::size_of::<u64>()];
    unsafe {
        let success = crate::vm::copy_user_to_kernel(&mut buffer, ptr);
        if !success {
            return Err(error::BAD_ARGUMENT);
        }
    }
    let mut v = vec![];
    let indices = buffer.chunks(core::mem::size_of::<u64>()).map(|x| {
        let bytes: [u8; core::mem::size_of::<u64>()] = x.try_into().unwrap();
        u64::from_ne_bytes(bytes) as usize
    });
    for i in indices {
        let Some(x) = arca.descriptors_mut().get_mut(i) else {
            return Err(error::BAD_INDEX);
        };
        let x = core::mem::take(x);
        v.push(x);
    }
    arca.descriptors_mut()[idx] = Value::Tree(v.into());
    Ok(len.try_into().expect("length was too long"))
}

fn replace_with<T>(x: &mut T, f: impl FnOnce(T) -> T) {
    unsafe {
        let old = core::ptr::read(x);
        let new = f(old);
        core::ptr::write(x, new);
    }
}

fn sys_continuation(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let idx = args[0] as usize;
    if idx >= arca.descriptors().len() {
        Err(error::BAD_INDEX)
    } else {
        replace_with(arca, |arca| {
            let (mut unloaded, cpu) = arca.unload_with_cpu();
            let mut copy = unloaded.clone();
            copy.registers_mut()[Register::RAX] = error::CONTINUED as u64;
            unloaded.descriptors_mut()[idx] = Value::Thunk(Thunk { arca: copy });
            unloaded.load(cpu)
        });
        Ok(0)
    }
}

fn sys_apply(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let lambda = args[0] as usize;
    let arg = args[1] as usize;

    let arg = arca
        .descriptors_mut()
        .get_mut(arg)
        .ok_or(error::BAD_INDEX)?;
    let x = core::mem::take(arg);

    let lambda = arca
        .descriptors_mut()
        .get_mut(lambda)
        .ok_or(error::BAD_INDEX)?;

    let l = core::mem::take(lambda);

    let Value::Lambda(l) = l else {
        return Err(error::BAD_TYPE);
    };

    let mut t = Value::Thunk(l.apply(x));
    core::mem::swap(&mut t, lambda);
    Ok(0)
}

#[allow(clippy::result_large_err)]
fn sys_prompt(args: [u64; 5], mut arca: LoadedArca) -> Result<LoadedValue, (LoadedArca, u32)> {
    let idx = args[0] as usize;
    if idx >= arca.descriptors().len() {
        Err((arca, error::BAD_INDEX))
    } else {
        arca.registers_mut()[Register::RAX] = 0;
        Ok(LoadedValue::Lambda(LoadedLambda { arca, idx }))
    }
}

#[allow(clippy::result_large_err)]
fn sys_perform(args: [u64; 5], arca: LoadedArca) -> Result<LoadedValue, (LoadedArca, u32)> {
    let src_idx = args[0] as usize;
    let dst_idx = args[1] as usize;
    if src_idx >= arca.descriptors().len() || dst_idx >= arca.descriptors().len() {
        Err((arca, error::BAD_INDEX))
    } else {
        let mut arca = arca.unload();
        arca.registers_mut()[Register::RAX] = 0;
        Ok(LoadedValue::Tree(vec![
            LoadedValue::Unloaded(arca.descriptors().get(src_idx).cloned().unwrap()),
            LoadedValue::Unloaded(Value::Lambda(Lambda { arca, idx: dst_idx })),
        ]))
    }
}

fn sys_tailcall(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let thunk = args[0] as usize;
    let thunk = arca
        .descriptors_mut()
        .get_mut(thunk)
        .ok_or(error::BAD_INDEX)?;
    let thunk = core::mem::take(thunk);

    let Value::Thunk(mut thunk) = thunk else {
        return Err(error::BAD_TYPE);
    };

    arca.swap(&mut thunk.arca);
    Ok(0)
}

fn sys_show(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let ptr = args[0] as usize;
    let len = args[1] as usize;
    let msg = unsafe {
        let mut buffer = Box::new_uninit_slice(len).assume_init();
        let success = crate::vm::copy_user_to_kernel(&mut buffer, ptr);
        if !success {
            return Err(error::BAD_ARGUMENT);
        }
        String::from_utf8_lossy(&buffer).into_owned()
    };
    let idx = args[2] as usize;
    if idx >= arca.descriptors().len() {
        return Err(error::BAD_INDEX);
    }
    let val = &arca.descriptors()[idx];
    log::info!("user message - \"{msg}\": {val:?}");
    Ok(0)
}

fn sys_log(args: [u64; 5], _: &mut LoadedArca) -> Result<u32, u32> {
    let ptr = args[0] as usize;
    let len = args[1] as usize;
    let msg = unsafe {
        let mut buffer = Box::new_uninit_slice(len).assume_init();
        let success = crate::vm::copy_user_to_kernel(&mut buffer, ptr);
        if !success {
            return Err(error::BAD_ARGUMENT);
        }
        String::from_utf8_lossy(&buffer).into_owned()
    };
    log::info!("user message - \"{msg}\"");
    Ok(0)
}
