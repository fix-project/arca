use core::{mem::MaybeUninit, ops::ControlFlow};

use alloc::collections::vec_deque::VecDeque;
use arcane::SyscallError;

use crate::{
    prelude::*,
    types::{
        arca::{DescriptorError, LoadedArca},
        internal,
    },
};

pub type Result<T> = core::result::Result<T, SyscallError>;

pub enum Action {
    Return(Value),
    Suspend,
    Call(Function),
    Capture,
}

pub fn handle_syscall(arca: &mut LoadedArca, argv: &mut VecDeque<Value>) -> ControlFlow<Action> {
    let regs = arca.registers();
    let num = regs[Register::RAX];
    let args = [
        regs[Register::RDI],
        regs[Register::RSI],
        regs[Register::RDX],
        regs[Register::R10],
        regs[Register::R8],
        regs[Register::R9],
    ];

    let result = match num as u32 {
        arcane::__NR_nop => Ok(0),
        arcane::__NR_drop => sys_drop(args, arca),
        arcane::__NR_clone => sys_clone(args, arca),

        arcane::__NR_exit => sys_exit(args, arca).map_break(Action::Return)?,
        arcane::__NR_get_argument => {
            if let Some(front) = argv.pop_front() {
                arca.descriptors_mut().insert(front)
            } else {
                arca.registers_mut()[Register::RAX] = (-(arcane::__ERR_interrupted as i32)) as u64;
                return ControlFlow::Break(Action::Suspend);
            }
        }
        arcane::__NR_length => sys_len(args, arca),
        arcane::__NR_get => sys_get(args, arca),
        arcane::__NR_set => sys_set(args, arca),
        arcane::__NR_read => sys_read(args, arca),
        arcane::__NR_write => sys_write(args, arca),
        arcane::__NR_type => sys_type(args, arca),

        arcane::__NR_create_word => sys_create_word(args, arca),
        arcane::__NR_create_blob => sys_create_blob(args, arca),
        arcane::__NR_create_tree => sys_create_tuple(args, arca),
        arcane::__NR_create_page => sys_create_page(args, arca),
        arcane::__NR_create_table => sys_create_table(args, arca),
        arcane::__NR_create_function => sys_create_function(args, arca),
        arcane::__NR_build => sys_build(args, arca)?,

        arcane::__NR_apply => sys_apply(args, arca),
        // arcane::__NR_force => sys_force(args, arca),
        arcane::__NR_map => sys_map(args, arca),
        arcane::__NR_mmap => sys_mmap(args, arca),
        arcane::__NR_mprotect => sys_mprotect(args, arca),
        arcane::__NR_compat_mmap => sys_compat_mmap(args, arca),

        arcane::__NR_call_with_current_continuation => {
            sys_call_with_current_continuation(args, arca)?
        }
        arcane::__NR_get_continuation => return ControlFlow::Break(Action::Capture),

        arcane::__NR_debug_show => sys_show(args, arca),
        arcane::__NR_debug_log => sys_log(args, arca),
        arcane::__NR_debug_log_int => sys_log_int(args, arca),

        _ => {
            log::error!("invalid syscall {num}");
            panic!("invalid syscall @ {:#x} ({args:#x?})", regs[Register::RIP]);
            // Err(SyscallError::BadSyscall)
        }
    };
    let regs = arca.registers_mut();
    if let Err(err) = result {
        log::debug!("system call {num} failed with {err:?}");
    }
    regs[Register::RAX] = match result {
        Ok(x) => x as u64,
        Err(e) => -(e as i64) as u64,
    };
    ControlFlow::Continue(())
}

pub fn sys_drop(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let src = args[0] as usize;
    arca.descriptors_mut().take(src)?;
    Ok(0)
}

pub fn sys_clone(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let src = args[0] as usize;
    let clone = arca.descriptors_mut().get(src)?.clone();
    arca.descriptors_mut().insert(clone)
}

pub fn sys_exit(args: [u64; 6], arca: &mut LoadedArca) -> ControlFlow<Value, Result<usize>> {
    let idx = args[0] as usize;
    let value = match arca.descriptors_mut().take(idx) {
        Ok(value) => value,
        Err(_) => {
            return ControlFlow::Break(
                arca::Function::symbolic("exit was called with a bad argument").into(),
            );
        }
    };
    ControlFlow::Break(value)
}

pub fn sys_len(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let idx = args[0] as usize;
    let ptr = args[1] as usize;
    let len = match arca.descriptors().get(idx)? {
        Value::Word(_) => core::mem::size_of::<u64>(),
        Value::Blob(blob) => blob.len(),
        Value::Tuple(tree) => tree.len(),
        Value::Page(page) => page.len(),
        Value::Table(table) => table.len(),
        _ => return Err(SyscallError::BadType),
    };
    copy_kernel_to_user(arca.cpu_mut(), ptr, &len.to_ne_bytes())?;
    Ok(0)
}

pub fn sys_set(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let target_idx = args[0] as usize;
    let inner_idx = args[1] as usize;
    let target = arca.descriptors().get(target_idx)?;
    let datatype = target.datatype();
    match datatype {
        DataType::Tuple => {
            let value_idx = args[2] as usize;
            let value = arca.descriptors_mut().take(value_idx)?;
            let Value::Tuple(tree) = arca.descriptors_mut().get_mut(target_idx)? else {
                unreachable!();
            };
            let value = tree.set(inner_idx, value);
            arca.descriptors_mut().insert(value)
        }
        DataType::Table => {
            let ptr = args[2] as usize;
            let mut entry: MaybeUninit<arcane::arca_entry> = MaybeUninit::uninit();
            copy_user_to_kernel(
                unsafe {
                    &mut *(entry.as_mut_ptr()
                        as *mut [MaybeUninit<u8>; core::mem::size_of::<arcane::arca_entry>()])
                },
                ptr,
            )?;
            let entry = unsafe { MaybeUninit::assume_init(entry) };
            let entry = read_entry(arca, entry)?;
            let Value::Table(table) = arca.descriptors_mut().get_mut(target_idx)? else {
                unreachable!();
            };
            let Ok(entry) = table.set(inner_idx, entry) else {
                todo!();
            };
            let entry = write_entry(arca, entry);
            copy_kernel_to_user(arca.cpu_mut(), ptr, unsafe {
                &*(&entry as *const arcane::arca_entry
                    as *const [u8; core::mem::size_of::<arcane::arca_entry>()])
            })?;
            Ok(0)
        }
        _ => Err(SyscallError::BadType),
    }
}

pub fn sys_get(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let target_idx = args[0] as usize;
    let inner_idx = args[1] as usize;
    let target = arca.descriptors_mut().get_mut(target_idx)?;
    match target {
        Value::Tuple(tree) => {
            let value = tree.get(inner_idx);
            arca.descriptors_mut().insert(value)
        }
        Value::Table(table) => {
            let ptr = args[2] as usize;
            let entry = table.get(inner_idx)?;
            let entry = write_entry(arca, entry);
            copy_kernel_to_user(arca.cpu_mut(), ptr, unsafe {
                &*(&entry as *const arcane::arca_entry
                    as *const [u8; core::mem::size_of::<arcane::arca_entry>()])
            })?;
            Ok(0)
        }
        _ => Err(SyscallError::BadType),
    }
}

pub fn sys_read(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let idx = args[0] as usize;
    let (descriptors, cpu) = arca.descriptors_and_cpu();
    match descriptors.get_mut(idx)? {
        Value::Word(word) => {
            let ptr = args[1] as usize;
            let word = word.read();
            copy_kernel_to_user(cpu, ptr, &word.to_ne_bytes())?;
            Ok(8)
        }
        Value::Blob(blob) => {
            let offset = args[1] as usize;
            let ptr = args[2] as usize;
            let len = args[3] as usize;
            let len = core::cmp::min(len, blob.len() - offset);
            copy_kernel_to_user(cpu, ptr, &blob.inner()[offset..offset + len])?;
            Ok(len)
        }
        Value::Page(page) => {
            let offset = args[1] as usize;
            let ptr = args[2] as usize;
            let len = args[3] as usize;
            let len = core::cmp::min(len, page.len() - offset);
            copy_kernel_to_user(cpu, ptr, &page.inner()[offset..offset + len])?;
            Ok(len)
        }
        Value::Function(_) => todo!(),
        _ => Err(SyscallError::BadType),
    }
}

pub fn sys_write(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let idx = args[0] as usize;
    match arca.descriptors_mut().get_mut(idx)? {
        Value::Blob(blob) => {
            let offset = args[1] as usize;
            let ptr = args[2] as usize;
            let len = args[3] as usize;
            let len = core::cmp::min(len, blob.len() - offset);
            copy_user_to_kernel_buf(&mut blob.inner_mut()[offset..offset + len], ptr)?;
            Ok(len)
        }
        Value::Page(page) => {
            let offset = args[1] as usize;
            let ptr = args[2] as usize;
            let len = args[3] as usize;
            let len = core::cmp::min(len, page.len() - offset);
            copy_user_to_kernel_buf(&mut page.inner_mut()[offset..offset + len], ptr)?;
            Ok(len)
        }
        Value::Function(_) => todo!(),
        _ => Err(SyscallError::BadType),
    }
}

pub fn sys_type(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let idx = args[0] as usize;
    let val = arca.descriptors().get(idx)?;
    let typ = match val {
        Value::Null(_) => arcane::__TYPE_null,
        Value::Word(_) => arcane::__TYPE_word,
        Value::Blob(_) => arcane::__TYPE_blob,
        Value::Tuple(_) => arcane::__TYPE_tuple,
        Value::Page(_) => arcane::__TYPE_page,
        Value::Table(_) => arcane::__TYPE_table,
        Value::Function(_) => arcane::__TYPE_function,
    };
    Ok(typ as usize)
}

pub fn sys_create_word(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let val = args[0];
    arca.descriptors_mut().insert(Word::new(val).into())
}

pub fn sys_create_blob(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let ptr = args[0] as usize;
    let len = args[1] as usize;
    let blob = copy_blob_from_user(ptr, len)?;
    arca.descriptors_mut().insert(blob.into())
}

pub fn sys_create_tuple(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let len = args[0] as usize;
    let buf = vec![Value::default(); len];
    let val = Value::Tuple(Tuple::from_inner(internal::Tuple::new(buf)));
    arca.descriptors_mut().insert(val)
}

pub fn sys_create_page(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let len = args[0] as usize;
    let val = Value::Page(Page::new(len));
    arca.descriptors_mut().insert(val)
}

pub fn sys_create_table(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let len = args[0] as usize;
    let val = Value::Table(Table::new(len));
    arca.descriptors_mut().insert(val)
}

pub fn sys_create_function(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let data = args[0] as usize;
    let data = arca.descriptors_mut().take(data)?;
    let result = Function::new(data)?;
    arca.descriptors_mut().insert(result.into())
}

enum Built {
    Value(Value),
    Call(Function),
}

pub fn sys_build(args: [u64; 6], arca: &mut LoadedArca) -> ControlFlow<Action, Result<usize>> {
    match build(args, arca) {
        Ok(Built::Value(value)) => ControlFlow::Continue(arca.descriptors_mut().insert(value)),
        Ok(Built::Call(function)) => ControlFlow::Break(Action::Call(function)),
        Err(error) => ControlFlow::Continue(Err(error)),
    }
}

fn build(args: [u64; 6], arca: &mut LoadedArca) -> Result<Built> {
    let ptr = args[0] as usize;
    let count = args[1] as usize;
    if count == 0 || count > 4096 {
        return Err(SyscallError::BadArgument);
    }
    let bytes = count
        .checked_mul(core::mem::size_of::<arcane::arca_build_step>())
        .ok_or(SyscallError::BadArgument)?;
    let mut instructions = vec![0u8; bytes];
    copy_user_to_kernel_buf(&mut instructions, ptr)?;
    let mut stack: Vec<Value> = Vec::new();
    for (index, chunk) in instructions
        .as_chunks::<{ core::mem::size_of::<arcane::arca_build_step>() }>()
        .0
        .iter()
        .enumerate()
    {
        let op = u64::from_ne_bytes(chunk[0..8].try_into().unwrap());
        let value = u64::from_ne_bytes(chunk[8..16].try_into().unwrap());
        let size = u64::from_ne_bytes(chunk[16..24].try_into().unwrap());
        if op == u64::from(arcane::ARCA_BUILD_CALL) {
            if index + 1 != count || stack.len() != 1 {
                return Err(SyscallError::BadArgument);
            }
            let function = stack
                .pop()
                .unwrap()
                .try_into()
                .map_err(|_| SyscallError::BadType)?;
            return Ok(Built::Call(function));
        }
        let next: Value = match op as u32 {
            arcane::ARCA_BUILD_TAKE if op == u64::from(arcane::ARCA_BUILD_TAKE) => {
                arca.descriptors_mut().take(value as usize)?
            }
            arcane::ARCA_BUILD_CLONE if op == u64::from(arcane::ARCA_BUILD_CLONE) => {
                arca.descriptors().get(value as usize)?.clone()
            }
            arcane::ARCA_BUILD_WORD if op == u64::from(arcane::ARCA_BUILD_WORD) => {
                Word::new(value).into()
            }
            arcane::ARCA_BUILD_BLOB if op == u64::from(arcane::ARCA_BUILD_BLOB) => {
                copy_blob_from_user(value as usize, size as usize)?.into()
            }
            arcane::ARCA_BUILD_TUPLE if op == u64::from(arcane::ARCA_BUILD_TUPLE) => {
                let count = size as usize;
                if count > stack.len() {
                    return Err(SyscallError::BadArgument);
                }
                let fields = stack.split_off(stack.len() - count);
                Tuple::from_inner(internal::Tuple::new(fields)).into()
            }
            arcane::ARCA_BUILD_FUNCTION if op == u64::from(arcane::ARCA_BUILD_FUNCTION) => {
                let data = stack.pop().ok_or(SyscallError::BadArgument)?;
                Function::new(data)
                    .map_err(|_| SyscallError::BadType)?
                    .into()
            }
            arcane::ARCA_BUILD_APPLY if op == u64::from(arcane::ARCA_BUILD_APPLY) => {
                let argument = stack.pop().ok_or(SyscallError::BadArgument)?;
                let function: Function = stack
                    .pop()
                    .ok_or(SyscallError::BadArgument)?
                    .try_into()
                    .map_err(|_| SyscallError::BadType)?;
                function.apply(argument).into()
            }
            _ => return Err(SyscallError::BadArgument),
        };
        stack.push(next);
    }
    if stack.len() != 1 {
        return Err(SyscallError::BadArgument);
    }
    Ok(Built::Value(stack.pop().unwrap()))
}

pub fn sys_apply(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let lambda = args[0] as usize;
    let arg = args[1] as usize;

    let f: Function = arca
        .descriptors_mut()
        .take(lambda)?
        .try_into()
        .map_err(|_| SyscallError::BadType)?;
    let x = arca.descriptors_mut().take(arg)?;

    let thunk = f.apply(x);
    arca.descriptors_mut().insert(thunk.into())
}

#[allow(unused)]
pub fn sys_force(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    // let lambda = args[0] as usize;
    //
    // let f: Function = arca
    //     .descriptors_mut()
    //     .take(lambda)?
    //     .try_into()
    //     .map_err(|_| SyscallError::BadType)?;
    //
    // let internal_function = f.into_inner();
    // let cpu = arca.cpu();
    // let result = internal_function.force_on(cpu);
    // let idx = arca.descriptors_mut().insert(result);
    // Ok(idx)
    //
    // ugh I give up
    Ok(0)
}

pub fn sys_map(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let table = args[0] as usize;
    let addr = args[1] as usize;
    let ptr = args[2] as usize;

    let mut entry: MaybeUninit<arcane::arca_entry> = MaybeUninit::uninit();
    copy_user_to_kernel(
        unsafe {
            &mut *(entry.as_mut_ptr()
                as *mut [MaybeUninit<u8>; core::mem::size_of::<arcane::arca_entry>()])
        },
        ptr,
    )?;
    let entry = unsafe { MaybeUninit::assume_init(entry) };
    let entry = read_entry(arca, entry)?;

    let table = match arca.descriptors_mut().get_mut(table)? {
        Value::Table(table) => table,
        Value::Function(_) => todo!("mapping into Function"),
        _ => return Err(SyscallError::BadType),
    };
    let entry = table
        .map(addr, entry)
        .map_err(|_| SyscallError::BadArgument)?;
    let entry = write_entry(arca, entry);
    copy_kernel_to_user(arca.cpu_mut(), ptr, unsafe {
        &*(&entry as *const arcane::arca_entry
            as *const [u8; core::mem::size_of::<arcane::arca_entry>()])
    })?;

    Ok(0)
}

pub fn sys_mmap(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let addr = args[0] as usize;
    let ptr = args[1] as usize;

    let mut entry: MaybeUninit<arcane::arca_entry> = MaybeUninit::uninit();
    copy_user_to_kernel(
        unsafe {
            &mut *(entry.as_mut_ptr()
                as *mut [MaybeUninit<u8>; core::mem::size_of::<arcane::arca_entry>()])
        },
        ptr,
    )?;
    let entry = unsafe { MaybeUninit::assume_init(entry) };
    let entry = read_entry(arca, entry)?;

    let entry = arca
        .cpu()
        .map(addr, entry)
        .map_err(|_| SyscallError::BadArgument)?;
    let entry = write_entry(arca, entry);
    copy_kernel_to_user(arca.cpu_mut(), ptr, unsafe {
        &*(&entry as *const arcane::arca_entry
            as *const [u8; core::mem::size_of::<arcane::arca_entry>()])
    })?;

    Ok(0)
}

pub fn sys_mprotect(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let addr = args[0] as usize;
    let mode = args[1] as u32;

    match mode {
        arcane::__MODE_none => {
            let _ = arca.cpu().map(addr, Entry::Null(4096));
        }
        arcane::__MODE_read_only => {
            let old = arca
                .cpu()
                .map(addr, Entry::Null(4096))
                .unwrap_or(Entry::Null(4096));
            let new = match old {
                arca::Entry::Null(_) => arca::Entry::ROPage(Page::new(4096)),
                arca::Entry::ROPage(_) => old,
                arca::Entry::RWPage(page) => arca::Entry::ROPage(page),
                arca::Entry::ROTable(_) => old,
                arca::Entry::RWTable(table) => arca::Entry::ROTable(table),
            };
            let _ = arca.cpu().map(addr, new);
        }
        arcane::__MODE_read_write => {
            let old = arca
                .cpu()
                .map(addr, Entry::Null(4096))
                .unwrap_or(Entry::Null(4096));
            let new = match old {
                arca::Entry::Null(_) => arca::Entry::RWPage(Page::new(4096)),
                arca::Entry::ROPage(page) => arca::Entry::RWPage(page),
                arca::Entry::RWPage(_) => old,
                arca::Entry::ROTable(table) => arca::Entry::RWTable(table),
                arca::Entry::RWTable(_) => old,
            };
            let _ = arca.cpu().map(addr, new);
        }
        _ => return Err(SyscallError::BadArgument),
    }
    Ok(0)
}

pub fn sys_compat_mmap(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let addr = args[0] as usize;
    let len = args[1] as usize;
    let mode = args[2] as u32;

    if !matches!(
        mode,
        arcane::__MODE_none | arcane::__MODE_read_only | arcane::__MODE_read_write
    ) {
        return Err(SyscallError::BadArgument);
    }

    if mode != arcane::__MODE_none {
        return arca
            .cpu_mut()
            .replace_zero_range(addr, len, mode == arcane::__MODE_read_write)
            .map_err(|_| SyscallError::BadArgument);
    }

    let mut p = addr;
    while p < addr + len {
        let remaining = addr + len - p;
        let span = [Page1GB::SIZE, Page2MB::SIZE, Page4KB::SIZE]
            .iter()
            .copied()
            .find(|span| p.is_multiple_of(*span) && (*span <= remaining || *span == Page4KB::SIZE))
            .ok_or(SyscallError::BadArgument)?;
        arca.cpu()
            .map(p, Entry::Null(span))
            .map_err(|_| SyscallError::BadArgument)?;
        p += span;
    }

    let size_mapped = p - addr;
    debug_assert_eq!(size_mapped, len.next_multiple_of(Page4KB::SIZE));
    Ok(size_mapped)
}

pub fn sys_call_with_current_continuation(
    args: [u64; 6],
    arca: &mut LoadedArca,
) -> ControlFlow<Action, Result<usize>> {
    let func = args[0] as usize;
    let func = match arca.descriptors_mut().take(func) {
        Ok(x) => x,
        Err(e) => return ControlFlow::Continue(Err(e)),
    };
    let Ok(func) = Function::try_from(func) else {
        return ControlFlow::Continue(Err(SyscallError::BadType));
    };
    ControlFlow::Break(Action::Call(func))
}

pub fn sys_show(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let ptr = args[0] as usize;
    let len = args[1] as usize;
    let idx = args[2] as usize;

    let mut buffer = Box::new_uninit_slice(len);
    copy_user_to_kernel(&mut buffer, ptr)?;
    let msg = String::from_utf8(unsafe { buffer.assume_init().into() })
        .map_err(|_| SyscallError::BadArgument)?;

    let val = &arca.descriptors().get(idx)?;
    log::warn!("\"{msg}\": {val:?}");
    Ok(0)
}

pub fn sys_log(args: [u64; 6], _: &mut LoadedArca) -> Result<usize> {
    let ptr = args[0] as usize;
    let len = args[1] as usize;

    let mut buffer = Box::new_uninit_slice(len);
    copy_user_to_kernel(&mut buffer, ptr)?;
    let msg = String::from_utf8(unsafe { buffer.assume_init().into() })
        .map_err(|_| SyscallError::BadArgument)?;

    log::warn!("\"{msg}\"");
    Ok(0)
}

pub fn sys_log_int(args: [u64; 6], _: &mut LoadedArca) -> Result<usize> {
    let ptr = args[0] as usize;
    let len = args[1] as usize;
    let val = args[2];

    let mut buffer = Box::new_uninit_slice(len);
    copy_user_to_kernel(&mut buffer, ptr)?;
    let msg = String::from_utf8(unsafe { buffer.assume_init().into() })
        .map_err(|_| SyscallError::BadArgument)?;

    log::warn!("\"{msg}\": {val} ({val:#x})");
    Ok(0)
}

fn copy_kernel_to_user(cpu: &mut Cpu, dst: usize, src: &[u8]) -> Result<()> {
    if crate::vm::copy_kernel_to_user(cpu, dst, src) {
        Ok(())
    } else {
        Err(SyscallError::BadArgument)
    }
}

fn copy_blob_from_user(ptr: usize, len: usize) -> Result<Blob> {
    let mut buffer = Box::new_uninit_slice(len);
    copy_user_to_kernel(&mut buffer, ptr)?;
    // A successful copy initializes the entire buffer.
    let buffer = unsafe { buffer.assume_init() };
    Ok(Blob::from_inner(internal::Blob::new(buffer)))
}

fn copy_user_to_kernel(dst: &mut [MaybeUninit<u8>], src: usize) -> Result<&mut [u8]> {
    crate::vm::copy_user_to_kernel(dst, src).ok_or(SyscallError::BadArgument)
}

fn copy_user_to_kernel_buf(dst: &mut [u8], src: usize) -> Result<&mut [u8]> {
    crate::vm::copy_user_to_kernel(
        unsafe { core::mem::transmute::<&mut [u8], &mut [MaybeUninit<u8>]>(dst) },
        src,
    )
    .ok_or(SyscallError::BadArgument)
}

fn read_entry(arca: &mut LoadedArca, entry: arcane::arca_entry) -> Result<Entry> {
    Ok(match entry {
        arcane::arca_entry {
            mode: arcane::__MODE_none,
            datatype: _,
            data,
        } => arca::Entry::Null(data),
        arcane::arca_entry {
            mode: arcane::__MODE_read_only,
            datatype: _,
            data,
        } => {
            let value = arca.descriptors_mut().take(data)?;
            match value {
                Value::Page(page) => arca::Entry::ROPage(page),
                Value::Table(table) => arca::Entry::ROTable(table),
                _ => return Err(SyscallError::BadType),
            }
        }
        arcane::arca_entry {
            mode: arcane::__MODE_read_write,
            datatype: _,
            data,
        } => {
            let value = arca.descriptors_mut().take(data)?;
            match value {
                Value::Page(page) => arca::Entry::RWPage(page),
                Value::Table(table) => arca::Entry::RWTable(table),
                _ => return Err(SyscallError::BadType),
            }
        }
        _ => return Err(SyscallError::BadArgument),
    })
}

fn write_entry(arca: &mut LoadedArca, entry: Entry) -> arcane::arca_entry {
    let (mode, datatype, value) = match entry {
        arca::Entry::Null(data) => {
            return arcane::arca_entry {
                mode: arcane::__MODE_none,
                datatype: arcane::__TYPE_null,
                data,
            };
        }
        arca::Entry::ROPage(x) => (arcane::__MODE_read_only, arcane::__TYPE_page, x.into()),
        arca::Entry::RWPage(x) => (arcane::__MODE_read_write, arcane::__TYPE_page, x.into()),
        arca::Entry::ROTable(x) => (arcane::__MODE_read_only, arcane::__TYPE_table, x.into()),
        arca::Entry::RWTable(x) => (arcane::__MODE_read_write, arcane::__TYPE_table, x.into()),
    };
    let index = arca.descriptors_mut().insert(value).unwrap();
    arcane::arca_entry {
        mode,
        datatype,
        data: index,
    }
}

impl From<DescriptorError> for SyscallError {
    fn from(value: DescriptorError) -> Self {
        match value {
            DescriptorError::AttemptToMutateNull => SyscallError::BadIndex,
            DescriptorError::OutOfBounds => SyscallError::BadIndex,
        }
    }
}

impl From<crate::types::Error> for SyscallError {
    fn from(value: crate::types::Error) -> Self {
        match value {
            crate::types::Error::InvalidTableEntry(_) => SyscallError::BadArgument,
            crate::types::Error::InvalidIndex(_) => SyscallError::BadIndex,
            crate::types::Error::InvalidValue => SyscallError::BadArgument,
            crate::types::Error::MapError => SyscallError::BadArgument,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mprotect_writable_leaf_preserves_readonly_siblings() {
        let address = 1 << 30;
        let mut table = Table::new(1 << 21);
        for (offset, value) in [(0, 0x29), (4096, 0x37)] {
            let mut page = Page::new(4096);
            page.write(0, &[value]);
            assert!(table.map(offset, Entry::RWPage(page)).is_ok());
        }
        let mut original = Arca::new();
        assert!(
            original
                .mappings_mut()
                .map(address, Entry::ROTable(table))
                .is_ok()
        );
        let mut cpu = CPU.borrow_mut();
        let mut loaded = original.clone().load(&mut cpu);
        for offset in [0, 4096] {
            let mut byte = [MaybeUninit::uninit()];
            assert!(crate::vm::copy_user_to_kernel(&mut byte, address + offset).is_some());
        }
        assert_eq!(
            sys_mprotect(
                [address as u64, arcane::__MODE_read_write as u64, 0, 0, 0, 0],
                &mut loaded,
            ),
            Ok(0)
        );
        assert!(crate::vm::copy_kernel_to_user(
            loaded.cpu_mut(),
            address,
            &[0x5a]
        ));
        assert!(!crate::vm::copy_kernel_to_user(
            loaded.cpu_mut(),
            address + 4096,
            &[0x44]
        ));
        let updated = loaded.unload();
        for (mapping, offset, expected) in [
            (&updated, 0, 0x5a),
            (&updated, 4096, 0x37),
            (&original, 0, 0x29),
            (&original, 4096, 0x37),
        ] {
            let mut byte = [0];
            assert_eq!(
                mapping.mappings().inner().read(address + offset, &mut byte),
                1
            );
            assert_eq!(byte, [expected]);
        }
    }

    // Verifies memory mapping rounds up to the nearest 4KB page without over allocation
    #[test]
    fn test_mmap_fits_request() {
        let len: usize = Page2MB::SIZE + Page4KB::SIZE;
        let mut cpu = CPU.borrow_mut();
        let mut loaded_arca = Arca::new().load(&mut cpu);
        let args = [0, len as u64, arcane::__MODE_read_write as u64, 0, 0, 0];
        sys_compat_mmap(args, &mut loaded_arca).expect("mmap failed");
        let mut arca = loaded_arca.unload();
        let mappings = arca.mappings_mut();

        for offset in (0..len).step_by(Page4KB::SIZE) {
            let Entry::RWPage(page) = mappings.unmap(offset).unwrap() else {
                panic!("expected writable page");
            };
            assert_eq!(page.len(), Page4KB::SIZE);
            let mut byte = [1];
            assert_eq!(page.read(0, &mut byte), 1);
            assert_eq!(byte, [0]);
        }
        assert!(matches!(mappings.unmap(len), Some(Entry::Null(_))));
    }
}
