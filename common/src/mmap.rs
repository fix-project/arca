use core::{
    num::NonZeroUsize,
    ops::{Deref, DerefMut},
};

use nix::sys::mman::{MapFlags, ProtFlags};

pub struct Mmap {
    ptr: *mut u8,
    len: usize,
}

impl Mmap {
    pub fn new(len: usize) -> Self {
        let ptr = unsafe {
            nix::sys::mman::mmap_anonymous(
                None,
                NonZeroUsize::new(len).unwrap(),
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_ANONYMOUS | MapFlags::MAP_SHARED | MapFlags::MAP_HUGE_2GB,
            )
            .unwrap()
            .as_ptr() as *mut u8
        };
        assert!(!ptr.is_null());
        log::debug!("mmapped {ptr:p}");
        Mmap { ptr, len }
    }

    pub fn into_raw(self) -> *mut [u8] {
        let slice = core::ptr::from_raw_parts_mut(self.ptr, self.len);
        core::mem::forget(self);
        slice
    }

    /// # Safety
    ///
    /// This pointer must have come from a call to [into_raw].
    pub unsafe fn from_raw(ptr: *mut [u8]) -> Self {
        let (ptr, len) = ptr.to_raw_parts();
        Self {
            ptr: ptr as *mut u8,
            len,
        }
    }
}

impl Drop for Mmap {
    fn drop(&mut self) {
        unsafe {
            assert_eq!(libc::munmap(self.ptr as _, self.len), 0);
            log::debug!("freed {:p}", self.ptr);
        }
    }
}

impl Deref for Mmap {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl DerefMut for Mmap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}
