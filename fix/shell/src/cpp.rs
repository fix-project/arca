use alloc::alloc::{Layout, alloc, dealloc};
use core::ffi::c_void;

#[unsafe(no_mangle)]
unsafe extern "C" fn fix_cpp_malloc(size: usize, alignment: usize) -> *mut c_void {
    let alignment = alignment.max(32);
    let Some(offset) = 24usize.checked_next_multiple_of(alignment) else {
        return core::ptr::null_mut();
    };
    let Some(total) = offset.checked_add(size.max(1)) else {
        return core::ptr::null_mut();
    };
    let Ok(layout) = Layout::from_size_align(total, alignment) else {
        return core::ptr::null_mut();
    };
    let base = unsafe { alloc(layout) };
    if base.is_null() {
        return core::ptr::null_mut();
    }
    let result = unsafe { base.add(offset) };
    unsafe {
        result.cast::<usize>().sub(3).write(base as usize);
        result.cast::<usize>().sub(2).write(total);
        result.cast::<usize>().sub(1).write(alignment);
    }
    result.cast()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn fix_cpp_free(pointer: *mut c_void) {
    if pointer.is_null() {
        return;
    }
    unsafe {
        let p = pointer.cast::<usize>();
        let base = p.sub(3).read() as *mut u8;
        let total = p.sub(2).read();
        let alignment = p.sub(1).read();
        let layout = Layout::from_size_align(total, alignment).unwrap();
        core::ptr::write_bytes(base, 0, total);
        dealloc(base, layout);
    }
}

#[unsafe(no_mangle)]
extern "C" fn fix_cpp_abort() -> ! {
    panic!("C++ runtime aborted")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn strcmp(mut left: *const u8, mut right: *const u8) -> i32 {
    loop {
        let a = unsafe { left.read() };
        let b = unsafe { right.read() };
        if a != b || a == 0 {
            return i32::from(a) - i32::from(b);
        }
        left = unsafe { left.add(1) };
        right = unsafe { right.add(1) };
    }
}

static mut FINALIZERS: alloc::vec::Vec<(
    unsafe extern "C" fn(*mut c_void),
    *mut c_void,
    *mut c_void,
)> = alloc::vec::Vec::new();

#[unsafe(no_mangle)]
unsafe extern "C" fn __cxa_atexit(
    callback: unsafe extern "C" fn(*mut c_void),
    value: *mut c_void,
    dso: *mut c_void,
) -> i32 {
    unsafe { (&mut *(&raw mut FINALIZERS)).push((callback, value, dso)) };
    0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn __cxa_finalize(dso: *mut c_void) {
    loop {
        let item = unsafe {
            let list = &mut *(&raw mut FINALIZERS);
            list.iter()
                .rposition(|item| dso.is_null() || item.2 == dso)
                .map(|index| list.remove(index))
        };
        let Some((callback, value, _)) = item else {
            break;
        };
        unsafe { callback(value) };
    }
}
