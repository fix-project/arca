use core::ops::ControlFlow;

use defs::error;

use crate::prelude::*;

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
    log::debug!("exited with syscall: {num:#?}({args:?})");
    let result = match num as u32 {
        defs::syscall::SYS_NOP => Ok(0),
        defs::syscall::SYS_DROP => sys_drop(args, arca),
        defs::syscall::SYS_CLONE => sys_clone(args, arca),
        defs::syscall::SYS_RESIZE => sys_resize(args, arca),

        defs::syscall::SYS_EXIT => sys_exit(args, arca)?,
        defs::syscall::SYS_LENGTH => sys_len(args, arca),
        defs::syscall::SYS_TAKE => sys_take(args, arca),
        defs::syscall::SYS_PUT => sys_put(args, arca),
        defs::syscall::SYS_READ => sys_read(args, arca),
        defs::syscall::SYS_TYPE => sys_type(args, arca),

        defs::syscall::SYS_CREATE_NULL => sys_create_null(args, arca),
        defs::syscall::SYS_CREATE_WORD => sys_create_word(args, arca),
        defs::syscall::SYS_CREATE_BLOB => sys_create_blob(args, arca),
        defs::syscall::SYS_CREATE_TREE => sys_create_tree(args, arca),

        defs::syscall::SYS_CAPTURE_CONTINUATION_THUNK => sys_continuation(args, arca),
        defs::syscall::SYS_CAPTURE_CONTINUATION_LAMBDA => sys_continuation_lambda(args, arca),
        // defs::syscall::SYS_RETURN_CONTINUATION => {
        //     return ControlFlow::Break(Value::Thunk(Thunk {
        //         arca: arca.unload(),
        //     }))
        // }
        defs::syscall::SYS_APPLY => sys_apply(args, arca),
        defs::syscall::SYS_RETURN_CONTINUATION_LAMBDA => {
            sys_return_continuation_lambda(args, arca)?
        }

        // defs::syscall::SYS_PERFORM_EFFECT => match sys_perform_effect(args, arca) {
        //     Ok(result) => return result,
        //     Err((a, e)) => {
        //         arca = a;
        //         Err(e)
        //     }
        // },
        defs::syscall::SYS_TAILCALL => sys_tailcall(args, arca),

        // defs::syscall::SYS_MAP_NEW_PAGES => sys_map_new_pages(args, arca),
        defs::syscall::SYS_DEBUG_SHOW => sys_show(args, arca),
        defs::syscall::SYS_DEBUG_LOG => sys_log(args, arca),
        _ => {
            log::error!("invalid syscall {num}");
            Err(error::ERROR_BAD_SYSCALL)
        }
    };
    let regs = arca.registers_mut();
    regs[Register::RAX] = match result {
        Ok(x) => x as u64,
        Err(e) => -(e as i64) as u64,
    };
    ControlFlow::Continue(())
}

// type ExitSyscall = fn([u64; 5], LoadedArca) -> Result<LoadedValue, (LoadedArca, u32)>;
// type Syscall = fn([u64; 5], &mut LoadedArca) -> Result<u32, u32>;

pub fn sys_create_null(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let idx = args[0] as usize;
    if let Some(x) = arca.descriptors_mut().get_mut(idx) {
        *x = Value::Null;
        Ok(0)
    } else {
        Err(error::ERROR_BAD_INDEX)
    }
}

pub fn sys_drop(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let src = args[0] as usize;
    *arca
        .descriptors_mut()
        .get_mut(src)
        .ok_or(error::ERROR_BAD_INDEX)? = Value::Null;
    Ok(0)
}

pub fn sys_clone(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let dst = args[0] as usize;
    let src = args[1] as usize;
    let clone = arca
        .descriptors()
        .get(src)
        .ok_or(error::ERROR_BAD_INDEX)?
        .clone();
    *arca
        .descriptors_mut()
        .get_mut(dst)
        .ok_or(error::ERROR_BAD_INDEX)? = clone;
    Ok(0)
}

pub fn sys_resize(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let len = args[0] as usize;
    arca.descriptors_mut().resize(len, Value::Null);
    Ok(0)
}

pub fn sys_exit(args: [u64; 5], arca: &mut LoadedArca) -> ControlFlow<Value, Result<u32, u32>> {
    let idx = args[0] as usize;
    if let Some(x) = arca.descriptors_mut().get_mut(idx) {
        let x = core::mem::take(x);
        arca.take();
        ControlFlow::Break(x)
    } else {
        log::warn!("exit failed with invalid index");
        ControlFlow::Continue(Err(error::ERROR_BAD_INDEX))
    }
}

pub fn sys_len(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let idx = args[0] as usize;
    let Some(val) = arca.descriptors().get(idx) else {
        return Err(error::ERROR_BAD_INDEX);
    };
    let ptr = args[1] as usize;
    let bytes: [u8; 8] = match val {
        Value::Blob(blob) => blob.len().to_ne_bytes(),
        Value::Tree(tree) => tree.len().to_ne_bytes(),
        _ => return Err(error::ERROR_BAD_TYPE),
    };

    unsafe {
        let success = crate::vm::copy_kernel_to_user(ptr, &bytes);
        if success {
            Ok(0)
        } else {
            Err(error::ERROR_BAD_ARGUMENT)
        }
    }
}

pub fn sys_take(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let new = args[0] as usize;
    let tree = args[1] as usize;
    let Some(tree) = arca.descriptors_mut().get_mut(tree) else {
        return Err(error::ERROR_BAD_INDEX);
    };
    let idx = args[2] as usize;
    let Value::Tree(tree) = tree else {
        return Err(error::ERROR_BAD_TYPE);
    };
    let Some(element) = tree.get_mut(idx) else {
        return Err(error::ERROR_BAD_ARGUMENT);
    };
    let old = core::mem::take(element);
    let Some(new) = arca.descriptors_mut().get_mut(new) else {
        return Err(error::ERROR_BAD_INDEX);
    };
    *new = old;

    Ok(0)
}

pub fn sys_put(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let tree_idx = args[0] as usize;
    let input = args[1] as usize;
    let index = args[2] as usize;

    let descriptors = arca.descriptors_mut();
    let tree = core::mem::take(
        descriptors
            .get_mut(tree_idx)
            .ok_or(error::ERROR_BAD_INDEX)?,
    );
    let Value::Tree(mut tree) = tree else {
        return Err(error::ERROR_BAD_TYPE);
    };
    let input = descriptors.get_mut(input).ok_or(error::ERROR_BAD_INDEX)?;

    core::mem::swap(tree.get_mut(index).ok_or(error::ERROR_BAD_ARGUMENT)?, input);
    *descriptors
        .get_mut(tree_idx)
        .ok_or(error::ERROR_BAD_INDEX)? = Value::Tree(tree);

    Ok(0)
}

pub fn sys_read(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    log::debug!("reading");
    let idx = args[0] as usize;
    let Some(val) = arca.descriptors_mut().get_mut(idx) else {
        return Err(error::ERROR_BAD_INDEX);
    };
    match val {
        Value::Word(word) => {
            log::debug!("reading word");
            let ptr = args[1] as usize;
            let bytes = word.read().to_ne_bytes();
            unsafe {
                let success = crate::vm::copy_kernel_to_user(ptr, &bytes);

                if success {
                    Ok(0)
                } else {
                    Err(error::ERROR_BAD_ARGUMENT)
                }
            }
        }
        Value::Blob(blob) => {
            log::debug!("reading blob");
            let ptr = args[1] as usize;
            let len = args[2] as usize;
            let len = core::cmp::min(len, blob.len());
            unsafe {
                let success = crate::vm::copy_kernel_to_user(ptr, &blob[..len]);

                if success {
                    Ok(0)
                } else {
                    Err(error::ERROR_BAD_ARGUMENT)
                }
            }
        }
        _ => {
            log::warn!("READ called with invalid type: {val:?}");
            Err(error::ERROR_BAD_TYPE)
        }
    }
}

pub fn sys_type(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let idx = args[0] as usize;
    let Some(val) = arca.descriptors().get(idx) else {
        return Err(error::ERROR_BAD_INDEX);
    };
    match val {
        Value::Null => Ok(defs::datatype::DATATYPE_NULL),
        Value::Error(_) => Ok(defs::datatype::DATATYPE_ERROR),
        Value::Word(_) => Ok(defs::datatype::DATATYPE_WORD),
        Value::Atom(_) => Ok(defs::datatype::DATATYPE_ATOM),
        Value::Blob(_) => Ok(defs::datatype::DATATYPE_BLOB),
        Value::Tree(_) => Ok(defs::datatype::DATATYPE_TREE),
        Value::Page(_) => Ok(defs::datatype::DATATYPE_PAGE),
        Value::Table(_) => Ok(defs::datatype::DATATYPE_TABLE),
        Value::Lambda(_) => Ok(defs::datatype::DATATYPE_LAMBDA),
        Value::Thunk(_) => Ok(defs::datatype::DATATYPE_THUNK),
    }
}

pub fn sys_create_word(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let idx = args[0] as usize;
    if idx >= arca.descriptors().len() {
        return Err(error::ERROR_BAD_INDEX);
    }
    let val = args[1];
    arca.descriptors_mut()[idx] = Value::Word(Word::new(val));
    Ok(0)
}

pub fn sys_create_blob(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let idx = args[0] as usize;
    if idx >= arca.descriptors().len() {
        return Err(error::ERROR_BAD_INDEX);
    }
    let ptr = args[1] as usize;
    let len = args[2] as usize;
    unsafe {
        let mut buffer = Box::new_uninit_slice(len).assume_init();
        let success = crate::vm::copy_user_to_kernel(&mut buffer, ptr);
        if !success {
            return Err(error::ERROR_BAD_ARGUMENT);
        }
        arca.descriptors_mut()[idx] = Value::Blob(buffer.into());
        Ok(len.try_into().expect("length was too long"))
    }
}

pub fn sys_create_tree(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let idx = args[0] as usize;
    let len = args[1] as usize;
    let buf = vec![Value::Null; len];
    let val = Value::Tree(Tree::new(buf));
    *arca
        .descriptors_mut()
        .get_mut(idx)
        .ok_or(error::ERROR_BAD_INDEX)? = val;
    Ok(0)
}

pub fn sys_continuation(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let idx = args[0] as usize;
    if idx >= arca.descriptors().len() {
        Err(error::ERROR_BAD_INDEX)
    } else {
        common::util::replace_with(arca, |arca| {
            let (mut unloaded, cpu) = arca.unload_with_cpu();
            let mut copy = unloaded.clone();
            copy.registers_mut()[Register::RAX] = -(error::ERROR_CONTINUED as i64) as u64;
            unloaded.descriptors_mut()[idx] = Value::Thunk(Thunk::new(copy));
            unloaded.load(cpu)
        });
        Ok(0)
    }
}

pub fn sys_continuation_lambda(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let idx = args[0] as usize;
    if idx >= arca.descriptors().len() {
        Err(error::ERROR_BAD_INDEX)
    } else {
        common::util::replace_with(arca, |arca| {
            let (mut unloaded, cpu) = arca.unload_with_cpu();
            let mut copy = unloaded.clone();
            copy.registers_mut()[Register::RAX] = -(error::ERROR_CONTINUED as i64) as u64;
            unloaded.descriptors_mut()[idx] = Value::Lambda(Lambda::new(copy, idx));
            unloaded.load(cpu)
        });
        Ok(0)
    }
}

pub fn sys_apply(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let output = args[0] as usize;
    let lambda = args[1] as usize;
    let arg = args[2] as usize;

    let arg = arca
        .descriptors_mut()
        .get_mut(arg)
        .ok_or(error::ERROR_BAD_INDEX)?;
    let x = core::mem::take(arg);

    let lambda = arca
        .descriptors_mut()
        .get_mut(lambda)
        .ok_or(error::ERROR_BAD_INDEX)?;

    let l = core::mem::take(lambda);

    let thunk = arca
        .descriptors_mut()
        .get_mut(output)
        .ok_or(error::ERROR_BAD_INDEX)?;

    let Value::Lambda(l) = l else {
        log::info!("lambda: {l:?}");
        return Err(error::ERROR_BAD_TYPE);
    };

    *thunk = Value::Thunk(l.apply(x));
    Ok(0)
}

pub fn sys_force(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let i = args[0] as usize;

    let thunk = arca
        .descriptors_mut()
        .get_mut(i)
        .ok_or(error::ERROR_BAD_INDEX)?;
    let thunk = core::mem::take(thunk);

    let Value::Thunk(thunk) = thunk else {
        log::warn!("tried to force non-thunk");
        return Err(error::ERROR_BAD_TYPE);
    };

    common::util::replace_with(arca, |arca| {
        let (mut arca, cpu) = arca.unload_with_cpu();
        let result = thunk.run_on(cpu);
        arca.descriptors_mut()[i] = result;
        arca.load(cpu)
    });
    Ok(0)
}

pub fn sys_return_continuation_lambda(
    args: [u64; 5],
    arca: &mut LoadedArca,
) -> ControlFlow<Value, Result<u32, u32>> {
    let idx = args[0] as usize;
    if idx >= arca.descriptors().len() {
        ControlFlow::Continue(Err(error::ERROR_BAD_INDEX))
    } else {
        arca.registers_mut()[Register::RAX] = 0;
        ControlFlow::Break(Value::Lambda(Lambda {
            arca: arca.take().into(),
            idx,
        }))
    }
}

#[allow(clippy::result_large_err)]
pub fn sys_perform_effect(args: [u64; 5], arca: LoadedArca) -> Result<Value, (LoadedArca, u32)> {
    let src_idx = args[0] as usize;
    let dst_idx = args[1] as usize;
    if src_idx >= arca.descriptors().len() || dst_idx >= arca.descriptors().len() {
        Err((arca, error::ERROR_BAD_INDEX))
    } else {
        let mut arca = arca.unload();
        arca.registers_mut()[Register::RAX] = 0;
        Ok(Value::Tree(Tree::new(vec![
            arca.descriptors().get(src_idx).cloned().unwrap(),
            Value::Lambda(Lambda::new(arca, dst_idx)),
        ])))
    }
}

pub fn sys_tailcall(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let thunk = args[0] as usize;
    let thunk = arca
        .descriptors_mut()
        .get_mut(thunk)
        .ok_or(error::ERROR_BAD_INDEX)?;
    let thunk = core::mem::take(thunk);

    let Value::Thunk(mut thunk) = thunk else {
        return Err(error::ERROR_BAD_TYPE);
    };

    arca.swap(&mut thunk.arca);
    Ok(0)
}

pub fn sys_map_new_pages(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let address = args[0] as usize;
    let count = args[1] as usize;

    for i in 0..count {
        let address = address + 4096 * i;
        let page = unsafe { UniquePage::<Page4KB>::new_zeroed_in(BuddyAllocator).assume_init() };
        arca.cpu().map_unique_4kb(address, page);
    }

    Ok(0)
}

pub fn sys_show(args: [u64; 5], arca: &mut LoadedArca) -> Result<u32, u32> {
    let ptr = args[0] as usize;
    let len = args[1] as usize;
    let msg = unsafe {
        let mut buffer = Box::new_uninit_slice(len).assume_init();
        let success = crate::vm::copy_user_to_kernel(&mut buffer, ptr);
        if !success {
            return Err(error::ERROR_BAD_ARGUMENT);
        }
        String::from_utf8_lossy(&buffer).into_owned()
    };
    let idx = args[2] as usize;
    if idx >= arca.descriptors().len() {
        return Err(error::ERROR_BAD_INDEX);
    }
    let val = &arca.descriptors()[idx];
    log::info!("user message - \"{msg}\": {val:?}");
    Ok(0)
}

pub fn sys_log(args: [u64; 5], _: &mut LoadedArca) -> Result<u32, u32> {
    let ptr = args[0] as usize;
    let len = args[1] as usize;
    let msg = unsafe {
        let mut buffer = Box::new_uninit_slice(len).assume_init();
        let success = crate::vm::copy_user_to_kernel(&mut buffer, ptr);
        if !success {
            return Err(error::ERROR_BAD_ARGUMENT);
        }
        String::from_utf8_lossy(&buffer).into_owned()
    };
    log::info!("user message - \"{msg}\"");
    Ok(0)
}
