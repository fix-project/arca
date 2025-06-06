use core::mem::MaybeUninit;

pub fn is_user(p: usize) -> bool {
    p & 0xFFFF800000000000 == 0
}

pub fn pa2ka<T>(p: usize) -> *mut T {
    (p | 0xFFFF800000000000) as *mut T
}

pub fn ka2pa<T>(p: *const T) -> usize {
    (p as usize) & !0xFFFF800000000000
}

pub const USER_MEMORY_LIMIT: usize = 0x00007fffffffffff;

extern "C" {
    fn copy_user_to_kernel_asm(dst: *mut (), src: usize, len: usize) -> bool;
    fn copy_kernel_to_user_asm(dst: usize, src: *const (), len: usize) -> bool;
}

pub(crate) fn copy_kernel_to_user(dst: usize, src: &[u8]) -> bool {
    let len = core::mem::size_of_val(src);
    let src = src.as_ptr();
    if (len <= USER_MEMORY_LIMIT) && (dst <= (USER_MEMORY_LIMIT - len)) {
        unsafe { copy_kernel_to_user_asm(dst, src as *const (), len) }
    } else {
        false
    }
}

pub(crate) fn copy_user_to_kernel(dst: &mut [MaybeUninit<u8>], src: usize) -> Option<&mut [u8]> {
    let ptr = dst.as_ptr();
    let len = dst.len();
    if (len <= USER_MEMORY_LIMIT) && (src <= (USER_MEMORY_LIMIT - len)) {
        unsafe {
            if copy_user_to_kernel_asm(ptr as *mut (), src, len) {
                Some(dst.assume_init_mut())
            } else {
                None
            }
        }
    } else {
        None
    }
}
