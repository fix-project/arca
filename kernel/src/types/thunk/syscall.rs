use core::{mem::MaybeUninit, ops::ControlFlow};

use defs::{error, SyscallError};

use crate::{
    prelude::*,
    types::{arca::DescriptorError, TypeError},
};

impl From<DescriptorError> for SyscallError {
    fn from(value: DescriptorError) -> Self {
        match value {
            DescriptorError::AttemptToMutateNull => SyscallError::BadIndex,
            DescriptorError::OutOfBounds => SyscallError::BadIndex,
        }
    }
}

impl From<TypeError> for SyscallError {
    fn from(_: TypeError) -> Self {
        SyscallError::BadType
    }
}

pub type Result<T> = core::result::Result<T, SyscallError>;

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

pub fn handle_syscall(arca: &mut LoadedArca) -> ControlFlow<Value> {
    let regs = arca.registers();
    let num = regs[Register::RDI];
    let args = [
        regs[Register::RSI],
        regs[Register::RDX],
        regs[Register::R10],
        regs[Register::R8],
        regs[Register::R9],
    ];

    if (num as u32) < defs::syscall::SYS_ERROR_RESET {
        log::debug!("exited with syscall: {num:#?}({args:?})");
    }
    let result = match num as u32 {
        defs::syscall::SYS_NOP => Ok(0),
        defs::syscall::SYS_DROP => sys_drop(args, arca),
        defs::syscall::SYS_CLONE => sys_clone(args, arca),

        defs::syscall::SYS_EXIT => sys_exit(args, arca)?,
        defs::syscall::SYS_LENGTH => sys_len(args, arca),
        defs::syscall::SYS_TAKE => sys_take(args, arca),
        defs::syscall::SYS_PUT => sys_put(args, arca),
        defs::syscall::SYS_READ => sys_read(args, arca),
        defs::syscall::SYS_TYPE => sys_type(args, arca),

        defs::syscall::SYS_CREATE_WORD => sys_create_word(args, arca),
        defs::syscall::SYS_CREATE_BLOB => sys_create_blob(args, arca),
        defs::syscall::SYS_CREATE_TREE => sys_create_tree(args, arca),

        defs::syscall::SYS_CAPTURE_CONTINUATION_THUNK => sys_continuation(args, arca),
        defs::syscall::SYS_CAPTURE_CONTINUATION_LAMBDA => sys_continuation_lambda(args, arca),
        defs::syscall::SYS_APPLY => sys_apply(args, arca),
        defs::syscall::SYS_RETURN_CONTINUATION_LAMBDA => {
            sys_return_continuation_lambda(args, arca)?
        }

        defs::syscall::SYS_TAILCALL => sys_tailcall(args, arca),

        defs::syscall::SYS_DEBUG_SHOW => sys_show(args, arca),
        defs::syscall::SYS_DEBUG_LOG => sys_log(args, arca),

        defs::syscall::SYS_ERROR_RESET => {
            arca.reset_error();
            Ok(0)
        }
        defs::syscall::SYS_ERROR_APPEND => sys_error_append(args, arca),
        defs::syscall::SYS_ERROR_RETURN => sys_error_return(args, arca)?,
        _ => {
            log::error!("invalid syscall {num}");
            Err(SyscallError::BadSyscall)
        }
    };
    let regs = arca.registers_mut();
    if (num as u32) < defs::syscall::SYS_ERROR_RESET {
        log::debug!("returning {result:?}");
    }
    regs[Register::RAX] = match result {
        Ok(x) => x as u64,
        Err(e) => -(e.code() as i64) as u64,
    };
    ControlFlow::Continue(())
}

pub fn sys_drop(args: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let src = args[0] as usize;
    arca.descriptors_mut().take(src)?;
    Ok(0)
}

pub fn sys_clone(args: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let src = args[0] as usize;
    let clone = arca.descriptors_mut().get(src)?.clone();
    Ok(arca.descriptors_mut().insert(clone))
}

pub fn sys_exit(args: [u64; 5], arca: &mut LoadedArca) -> ControlFlow<Value, Result<usize>> {
    let idx = args[0] as usize;
    let value = match arca.descriptors_mut().take(idx) {
        Ok(value) => value,
        Err(err) => return ControlFlow::Continue(Err(err.into())),
    };
    ControlFlow::Break(value)
}

pub fn sys_len(args: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let idx = args[0] as usize;
    let ptr = args[1] as usize;
    let len = match arca.descriptors().get(idx)? {
        Value::Blob(blob) => blob.len(),
        Value::Tree(tree) => tree.len(),
        Value::Page(page) => page.size(),
        Value::Table(table) => table.size(),
        _ => return Err(SyscallError::BadType),
    };
    copy_kernel_to_user(ptr, &len.to_ne_bytes())?;
    Ok(0)
}

pub fn sys_take(args: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let target_idx = args[0] as usize;
    let inner_idx = args[1] as usize;
    let target = arca.descriptors_mut().get_mut(target_idx)?;
    match target {
        Value::Tree(tree) => {
            let value = tree.take(inner_idx);
            Ok(arca.descriptors_mut().insert(value))
        }
        Value::Table(table) => {
            let permissions = args[2] as usize;
            let entry = table.take(inner_idx);
            let (value, mode): (Value, u64) = match entry {
                arca::Entry::Null(x) => (x.into(), 0),
                arca::Entry::ROPage(x) => (x.into(), 1),
                arca::Entry::RWPage(x) => (x.into(), 2),
                arca::Entry::ROTable(x) => (x.into(), 3),
                arca::Entry::RWTable(x) => (x.into(), 4),
            };
            let index = arca.descriptors_mut().insert(value);
            copy_kernel_to_user(permissions, &mode.to_ne_bytes())?;
            Ok(index)
        }
        _ => Err(SyscallError::BadType),
    }
}

pub fn sys_put(args: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let target_idx = args[0] as usize;
    let inner_idx = args[1] as usize;
    let value_idx = args[2] as usize;
    let value = arca.descriptors_mut().take(value_idx)?;
    let target = arca.descriptors_mut().get_mut(target_idx)?;
    match target {
        Value::Tree(tree) => {
            let value = tree.put(inner_idx, value);
            Ok(arca.descriptors_mut().insert(value))
        }
        Value::Table(table) => {
            let mode_ptr = args[3] as usize;
            let mut mode: [MaybeUninit<u8>; 8] = MaybeUninit::uninit().transpose();
            copy_user_to_kernel(&mut mode, mode_ptr)?;
            let mode = unsafe { u64::from_ne_bytes(MaybeUninit::array_assume_init(mode)) };
            let entry = match mode {
                0 => arca::Entry::Null(Null),
                1 => arca::Entry::ROPage(value.try_into()?),
                2 => arca::Entry::RWPage(value.try_into()?),
                3 => arca::Entry::ROTable(value.try_into()?),
                4 => arca::Entry::RWTable(value.try_into()?),
                _ => unreachable!(),
            };
            let Ok(entry) = table.put(inner_idx, entry) else {
                todo!();
            };
            let (value, mode): (Value, u64) = match entry {
                arca::Entry::Null(x) => (x.into(), 0),
                arca::Entry::ROPage(x) => (x.into(), 1),
                arca::Entry::RWPage(x) => (x.into(), 2),
                arca::Entry::ROTable(x) => (x.into(), 3),
                arca::Entry::RWTable(x) => (x.into(), 4),
            };
            let index = arca.descriptors_mut().insert(value);
            copy_kernel_to_user(mode_ptr, &mode.to_ne_bytes())?;
            Ok(index)
        }
        _ => Err(SyscallError::BadType),
    }
}

pub fn sys_read(args: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let idx = args[0] as usize;
    match arca.descriptors_mut().get_mut(idx)? {
        Value::Word(word) => {
            let ptr = args[1] as usize;
            let word = word.read();
            copy_kernel_to_user(ptr, &word.to_ne_bytes())?;
            Ok(0)
        }
        Value::Error(error) => {
            let error = core::mem::replace(error, Error::new(Value::Null));
            let idx = arca.descriptors_mut().insert(error.read());
            Ok(idx)
        }
        Value::Blob(blob) => {
            let ptr = args[1] as usize;
            let len = args[2] as usize;
            let len = core::cmp::min(len, blob.len());
            copy_kernel_to_user(ptr, &blob[..len])?;
            Ok(0)
        }
        Value::Page(_) => todo!(),
        Value::Lambda(_) => todo!(),
        Value::Thunk(_) => todo!(),
        _ => return Err(SyscallError::BadType),
    }
}

pub fn sys_type(args: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let idx = args[0] as usize;
    let val = arca.descriptors().get(idx)?;
    let typ = match val {
        Value::Null => defs::datatype::DATATYPE_NULL,
        Value::Error(_) => defs::datatype::DATATYPE_ERROR,
        Value::Word(_) => defs::datatype::DATATYPE_WORD,
        Value::Atom(_) => defs::datatype::DATATYPE_ATOM,
        Value::Blob(_) => defs::datatype::DATATYPE_BLOB,
        Value::Tree(_) => defs::datatype::DATATYPE_TREE,
        Value::Page(_) => defs::datatype::DATATYPE_PAGE,
        Value::Table(_) => defs::datatype::DATATYPE_TABLE,
        Value::Lambda(_) => defs::datatype::DATATYPE_LAMBDA,
        Value::Thunk(_) => defs::datatype::DATATYPE_THUNK,
    };
    Ok(typ as usize)
}

pub fn sys_create_word(args: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let val = args[0];
    Ok(arca.descriptors_mut().insert(Word::new(val).into()))
}

pub fn sys_create_blob(args: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let ptr = args[0] as usize;
    let len = args[1] as usize;
    unsafe {
        let mut buffer = Box::new_uninit_slice(len);
        copy_user_to_kernel(&mut buffer, ptr)?;
        let buffer = buffer.assume_init();
        Ok(arca.descriptors_mut().insert(Value::Blob(buffer.into())))
    }
}

pub fn sys_create_tree(args: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let len = args[0] as usize;
    let buf = vec![Value::Null; len];
    let val = Value::Tree(Tree::new(buf));
    Ok(arca.descriptors_mut().insert(val))
}

pub fn sys_continuation(_: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let mut idx = 0;
    common::util::replace_with(arca, |arca| {
        let (mut unloaded, cpu) = arca.unload_with_cpu();
        let mut copy = unloaded.clone();
        copy.registers_mut()[Register::RAX] = -(error::ERROR_CONTINUED as i64) as u64;
        idx = unloaded
            .descriptors_mut()
            .insert(Value::Thunk(Thunk::new(copy)));
        unloaded.load(cpu)
    });
    Ok(idx)
}

pub fn sys_continuation_lambda(_: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let mut idx = 0;
    common::util::replace_with(arca, |arca| {
        let (mut unloaded, cpu) = arca.unload_with_cpu();
        let mut copy = unloaded.clone();
        copy.registers_mut()[Register::RAX] = -(error::ERROR_CONTINUED as i64) as u64;
        idx = unloaded
            .descriptors_mut()
            .insert(Value::Lambda(Lambda::new(copy)));
        unloaded.load(cpu)
    });
    Ok(idx)
}

pub fn sys_apply(args: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let lambda = args[0] as usize;
    let arg = args[1] as usize;

    let lambda: Lambda = arca.descriptors_mut().take(lambda)?.try_into()?;
    let arg = arca.descriptors_mut().take(arg)?;

    let thunk = lambda.apply(arg);
    Ok(arca.descriptors_mut().insert(thunk.into()))
}

pub fn sys_force(args: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let thunk = args[0] as usize;
    let thunk: Thunk = arca.descriptors_mut().take(thunk)?.try_into()?;

    let mut idx = 0;
    common::util::replace_with(arca, |arca| {
        let (mut arca, cpu) = arca.unload_with_cpu();
        let result = thunk.run_on(cpu);
        idx = arca.descriptors_mut().insert(result);
        arca.load(cpu)
    });
    Ok(idx)
}

pub fn sys_return_continuation_lambda(
    _: [u64; 5],
    arca: &mut LoadedArca,
) -> ControlFlow<Value, Result<usize>> {
    arca.registers_mut()[Register::RAX] = 0;
    ControlFlow::Break(Value::Lambda(Lambda::new(arca.take())))
}

pub fn sys_tailcall(args: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let thunk = args[0] as usize;
    let mut thunk: Thunk = arca.descriptors_mut().take(thunk)?.try_into()?;
    arca.swap(&mut thunk.arca);
    Ok(0)
}

pub fn sys_show(args: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let ptr = args[0] as usize;
    let len = args[1] as usize;
    let idx = args[2] as usize;

    let mut buffer = Box::new_uninit_slice(len);
    copy_user_to_kernel(&mut buffer, ptr)?;
    let msg = String::from_utf8(unsafe { buffer.assume_init().into() })
        .map_err(|_| SyscallError::BadArgument)?;

    let val = &arca.descriptors().get(idx)?;
    log::info!("user message - \"{msg}\": {val:?}");
    Ok(0)
}

pub fn sys_log(args: [u64; 5], _: &mut LoadedArca) -> Result<usize> {
    let ptr = args[0] as usize;
    let len = args[1] as usize;

    let mut buffer = Box::new_uninit_slice(len);
    copy_user_to_kernel(&mut buffer, ptr)?;
    let msg = String::from_utf8(unsafe { buffer.assume_init().into() })
        .map_err(|_| SyscallError::BadArgument)?;

    log::info!("user message - \"{msg}\"");
    Ok(0)
}

pub fn sys_error_append(args: [u64; 5], arca: &mut LoadedArca) -> Result<usize> {
    let ptr = args[0] as usize;
    let len = args[1] as usize;
    let mut buf = Box::new_uninit_slice(len);
    let buf = copy_user_to_kernel(&mut buf, ptr)?;
    let s = core::str::from_utf8(buf).map_err(|_| SyscallError::BadArgument)?;
    arca.error_buffer_mut().push_str(s);
    Ok(0)
}

pub fn sys_error_return(_: [u64; 5], arca: &mut LoadedArca) -> ControlFlow<Value, Result<usize>> {
    let buffer = core::mem::take(arca.error_buffer_mut());
    let blob = Blob::string(buffer);
    ControlFlow::Break(Error::new(blob.into()).into())
}
