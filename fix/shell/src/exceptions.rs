use alloc::vec::Vec;

static mut PAYLOAD: Vec<u8> = Vec::new();

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wasm_rt_store_exception(size: u32, values: *const u8) -> *mut u8 {
    let payload = unsafe { &mut *(&raw mut PAYLOAD) };
    let size = size as usize;
    if payload
        .try_reserve_exact(size.saturating_sub(payload.len()))
        .is_err()
    {
        return core::ptr::null_mut();
    }
    if size != 0 {
        assert!(!values.is_null());
        unsafe { core::ptr::copy(values, payload.as_mut_ptr(), size) };
    }
    unsafe { payload.set_len(size) };
    payload.as_mut_ptr()
}
