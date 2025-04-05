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

pub(crate) unsafe fn copy_kernel_to_user(dst: usize, src: &[u8]) -> bool {
    let len = core::mem::size_of_val(src);
    let src = src.as_ptr();
    if (len <= USER_MEMORY_LIMIT) && (dst <= (USER_MEMORY_LIMIT - len)) {
        copy_kernel_to_user_asm(dst, src as *const (), len)
    } else {
        false
    }
}

pub(crate) unsafe fn copy_user_to_kernel(dst: &mut [u8], src: usize) -> bool {
    let len = core::mem::size_of_val(dst);
    let dst = dst.as_mut_ptr();
    if (len <= USER_MEMORY_LIMIT) && (src <= (USER_MEMORY_LIMIT - len)) {
        copy_user_to_kernel_asm(dst as *mut (), src, len)
    } else {
        false
    }
}
