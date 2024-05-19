pub fn pa2ka<T>(p: *const T) -> *const T {
    ((p as usize) | 0xFFFF800000000000) as *const T
}

pub fn ka2pa<T>(p: *const T) -> *const T {
    ((p as usize) & !0xFFFF800000000000) as *const T
}

pub fn pa2ka_mut<T>(p: *mut T) -> *mut T {
    ((p as usize) | 0xFFFF800000000000) as *mut T
}

pub fn ka2pa_mut<T>(p: *mut T) -> *mut T {
    ((p as usize) & !0xFFFF800000000000) as *mut T
}
