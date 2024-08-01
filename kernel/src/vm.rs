pub fn pa2ka<T>(p: usize) -> *mut T {
    (p | 0xFFFF800000000000) as *mut T
}

pub fn ka2pa<T>(p: *const T) -> usize {
    (p as usize) & !0xFFFF800000000000
}
