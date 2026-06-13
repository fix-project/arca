//! Shared memory region abstraction for pipe endpoints.

/// A cheap, cloneable handle to a shared memory region mapped into both pipe
/// endpoints' address spaces.
///
/// This is a *handle*, not necessarily an owner in the RAII sense — cloning
/// yields another handle to the *same* underlying bytes. How the region is
/// mapped (hypervisor page, POSIX shm, `mmap`) and whether dropping the last
/// handle unmaps it is the concrete implementation's concern.
///
/// The trait requires [`Clone`] so each half of a split pipe (see
/// [`super::bidirectional_pipe`]) can own its own handle and keep the mapping
/// alive independently — this is what lets the pipe carry no lifetime.
pub trait SharedMemoryRegion: Clone {
    /// Pointer to the start of the region. Valid for at least [`len`](Self::len) bytes.
    ///
    /// # Safety contract for implementors
    /// Pipe ends cache this pointer for their whole lifetime, so an
    /// implementation MUST guarantee that the returned address points to the
    /// same allocation — valid for at least `len()` bytes of reads/writes — for
    /// as long as *any* handle (the original or any clone) is alive. An `R` that
    /// relocates or invalidates the backing memory while a handle lives (e.g. a
    /// clone that re-maps to a new address) makes the ring ends' raw pointers
    /// dangling and is unsound.
    fn as_ptr(&self) -> *mut u8;

    /// Length of the region in bytes.
    fn len(&self) -> u64;

    /// True if the region has zero length.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A [`SharedMemoryRegion`] that is nothing but a raw `(ptr, len)` pair with no
/// ownership semantics: dropping it does nothing.
///
/// This is the Arca-side / test implementation — the memory is mapped by the
/// hypervisor and is never unmapped by this type, so cloning (a plain `Copy`)
/// is free. The host (vmm) side can provide a different implementation backed
/// by an `mmap`/`Arc` handle that manages unmapping on drop of the last clone.
#[derive(Clone, Copy)]
pub struct RawSharedMemoryRegion {
    ptr: *mut u8,
    len: u64,
}

// SAFETY: `RawSharedMemoryRegion` is a bare address + length into memory that is
// intentionally shared across threads/address spaces. It carries no ownership,
// so moving or sharing the handle itself is sound; correct concurrent *access*
// to the bytes is enforced by the SPSC ring discipline layered on top, not by
// this handle.
unsafe impl Send for RawSharedMemoryRegion {}
unsafe impl Sync for RawSharedMemoryRegion {}

impl RawSharedMemoryRegion {
    /// Create a region handle from a raw pointer.
    ///
    /// This is the one and only unsafe entry point for the pipe library.
    ///
    /// # Safety
    /// - `ptr` must point to a valid, read-write region of at least `len` bytes.
    /// - The memory must remain valid for as long as any handle (or any pipe /
    ///   pipe-half derived from it) is alive.
    /// - The memory must be shared between both sides of the pipe (e.g. via
    ///   hypervisor page mapping or POSIX shared memory).
    /// - The memory must be zero-initialized before the first pipe is created from it.
    pub unsafe fn from_raw(ptr: *mut u8, len: u64) -> Self {
        Self { ptr, len }
    }
}

impl SharedMemoryRegion for RawSharedMemoryRegion {
    fn as_ptr(&self) -> *mut u8 {
        self.ptr
    }

    fn len(&self) -> u64 {
        self.len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_raw_stores_ptr_and_len() {
        let mut buf = [0u8; 16];
        let region = unsafe { RawSharedMemoryRegion::from_raw(buf.as_mut_ptr(), buf.len() as u64) };
        assert_eq!(region.as_ptr(), buf.as_mut_ptr());
        assert_eq!(region.len(), 16);
        assert!(!region.is_empty());
    }

    #[test]
    fn zero_length_is_empty() {
        let region = unsafe { RawSharedMemoryRegion::from_raw(core::ptr::null_mut(), 0) };
        assert_eq!(region.len(), 0);
        assert!(region.is_empty());
    }

    #[test]
    fn clone_points_to_same_memory() {
        let mut buf = [0u8; 8];
        let a = unsafe { RawSharedMemoryRegion::from_raw(buf.as_mut_ptr(), 8) };
        let b = a; // Copy handle — clone points at the same bytes
        assert_eq!(a.as_ptr(), b.as_ptr());
        assert_eq!(a.len(), b.len());
    }
}
