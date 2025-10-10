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

pub fn handle_syscall(arca: &mut LoadedArca, argv: &mut VecDeque<Value>) -> ControlFlow<Value> {
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

        arcane::__NR_exit => sys_exit(args, arca)?,
        arcane::__NR_get_argument => {
            if let Some(front) = argv.pop_front() {
                let idx = arca.descriptors_mut().insert(front);
                Ok(idx)
            } else {
                arca.registers_mut()[Register::RAX] = (-(arcane::__ERR_interrupted as i32)) as u64;
                let arca = arca.take();
                let argv = core::mem::take(argv);
                return ControlFlow::Break(Value::Function(Function::from_inner(
                    internal::Function::arcane_with_args(arca, argv),
                )));
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

        arcane::__NR_apply => sys_apply(args, arca),
        arcane::__NR_map => sys_map(args, arca),
        arcane::__NR_mmap => sys_mmap(args, arca),
        arcane::__NR_mprotect => sys_mprotect(args, arca),

        arcane::__NR_call_with_current_continuation => {
            sys_call_with_current_continuation(args, arca)?
        }
        arcane::__NR_get_continuation => sys_get_continuation(args, arca),

        arcane::__NR_debug_show => sys_show(args, arca),
        arcane::__NR_debug_log => sys_log(args, arca),
        arcane::__NR_debug_log_int => sys_log_int(args, arca),

        _ => {
            log::error!("invalid syscall {num}");
            panic!("invalid syscall @ {:#x}", regs[Register::RIP]);
            // Err(SyscallError::BadSyscall)
        }
    };
    let regs = arca.registers_mut();
    if let Err(err) = result {
        // log::warn!("system call {num} failed with {err:?}");
        panic!("system call {num} failed with {err:?}");
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
    Ok(arca.descriptors_mut().insert(clone))
}

pub fn sys_exit(args: [u64; 6], arca: &mut LoadedArca) -> ControlFlow<Value, Result<usize>> {
    let idx = args[0] as usize;
    let value = match arca.descriptors_mut().take(idx) {
        Ok(value) => value,
        Err(err) => return ControlFlow::Continue(Err(err.into())),
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
    copy_kernel_to_user(ptr, &len.to_ne_bytes())?;
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
            let Value::Tuple(ref mut tree) = arca.descriptors_mut().get_mut(target_idx)? else {
                unreachable!();
            };
            let value = tree.set(inner_idx, value);
            Ok(arca.descriptors_mut().insert(value))
        }
        DataType::Table => {
            let ptr = args[3] as usize;
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
            let Value::Table(ref mut table) = arca.descriptors_mut().get_mut(target_idx)? else {
                unreachable!();
            };
            let Ok(entry) = table.set(inner_idx, entry) else {
                todo!();
            };
            let entry = write_entry(arca, entry);
            copy_kernel_to_user(ptr, unsafe {
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
            Ok(arca.descriptors_mut().insert(value))
        }
        Value::Table(table) => {
            let ptr = args[2] as usize;
            let entry = table.get(inner_idx)?;
            let entry = write_entry(arca, entry);
            copy_kernel_to_user(ptr, unsafe {
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
    match arca.descriptors_mut().get_mut(idx)? {
        Value::Word(word) => {
            let ptr = args[1] as usize;
            let word = word.read();
            copy_kernel_to_user(ptr, &word.to_ne_bytes())?;
            Ok(8)
        }
        Value::Blob(blob) => {
            let offset = args[1] as usize;
            let ptr = args[2] as usize;
            let len = args[3] as usize;
            let len = core::cmp::min(len, blob.len() - offset);
            copy_kernel_to_user(ptr, &blob.inner()[offset..offset + len])?;
            Ok(len)
        }
        Value::Page(page) => {
            let offset = args[1] as usize;
            let ptr = args[2] as usize;
            let len = args[3] as usize;
            let len = core::cmp::min(len, page.len() - offset);
            copy_kernel_to_user(ptr, &page.inner()[offset..offset + len])?;
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
    Ok(arca.descriptors_mut().insert(Word::new(val).into()))
}

pub fn sys_create_blob(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let ptr = args[0] as usize;
    let len = args[1] as usize;
    let mut buffer = Box::new_uninit_slice(len);
    let buffer = copy_user_to_kernel(&mut buffer, ptr)?;
    Ok(arca
        .descriptors_mut()
        .insert(Value::Blob(Blob::new(buffer))))
}

pub fn sys_create_tuple(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let len = args[0] as usize;
    let buf = vec![Value::default(); len];
    let val = Value::Tuple(Tuple::from_inner(internal::Tuple::new(buf)));
    Ok(arca.descriptors_mut().insert(val))
}

pub fn sys_create_page(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let len = args[0] as usize;
    let val = Value::Page(Page::new(len));
    Ok(arca.descriptors_mut().insert(val))
}

pub fn sys_create_table(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let len = args[0] as usize;
    let val = Value::Table(Table::new(len));
    Ok(arca.descriptors_mut().insert(val))
}

pub fn sys_create_function(args: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let data = args[0] as usize;
    let data = arca.descriptors_mut().take(data)?;
    let result = Function::new(data)?;
    Ok(arca.descriptors_mut().insert(result.into()))
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
    let idx = arca.descriptors_mut().insert(thunk.into());
    Ok(idx)
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
    copy_kernel_to_user(ptr, unsafe {
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
    copy_kernel_to_user(ptr, unsafe {
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

pub fn sys_call_with_current_continuation(
    args: [u64; 6],
    arca: &mut LoadedArca,
) -> ControlFlow<Value, Result<usize>> {
    let func = args[0] as usize;
    let func = match arca.descriptors_mut().take(func) {
        Ok(x) => x,
        Err(e) => return ControlFlow::Continue(Err(e.into())),
    };
    let Ok(func) = Function::try_from(func) else {
        return ControlFlow::Continue(Err(SyscallError::BadType));
    };
    let k: Value = Function::from_inner(internal::Function::arcane_with_args(
        arca.take(),
        Default::default(),
    ))
    .into();
    ControlFlow::Break(func.apply(k).into())
}

pub fn sys_get_continuation(_: [u64; 6], arca: &mut LoadedArca) -> Result<usize> {
    let mut unloaded = arca.take();
    let mut new = unloaded.clone();
    new.registers_mut()[Register::RAX] = -(arcane::__ERR_interrupted as i64) as u64;
    let k: Value = Function::from_inner(internal::Function::arcane_with_args(
        new,
        Default::default(),
    ))
    .into();
    arca.swap(&mut unloaded);
    Ok(arca.descriptors_mut().insert(k))
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

fn copy_kernel_to_user(dst: usize, src: &[u8]) -> Result<()> {
    if crate::vm::copy_kernel_to_user(dst, src) {
        Ok(())
    } else {
        Err(SyscallError::BadArgument)
    }
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
            }
        }
        arca::Entry::ROPage(x) => (arcane::__MODE_read_only, arcane::__TYPE_page, x.into()),
        arca::Entry::RWPage(x) => (arcane::__MODE_read_write, arcane::__TYPE_page, x.into()),
        arca::Entry::ROTable(x) => (arcane::__MODE_read_only, arcane::__TYPE_table, x.into()),
        arca::Entry::RWTable(x) => (arcane::__MODE_read_write, arcane::__TYPE_table, x.into()),
    };
    let index = arca.descriptors_mut().insert(value);
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
        }
    }
}
