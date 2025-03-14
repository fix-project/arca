/// # Safety
/// This trait should only be implemented by types which have a stable, guaranteed in-memory
/// representation with no external references.  This means implementors can be safely sent via IPC
/// without serialization or deserialization.
pub unsafe trait Sendable: core::fmt::Debug + Sized {}

unsafe impl Sendable for u8 {}
unsafe impl Sendable for u16 {}
unsafe impl Sendable for u32 {}
unsafe impl Sendable for u64 {}
unsafe impl Sendable for usize {}
unsafe impl Sendable for i8 {}
unsafe impl Sendable for i16 {}
unsafe impl Sendable for i32 {}
unsafe impl Sendable for i64 {}
unsafe impl Sendable for isize {}

unsafe impl Sendable for bool {}
unsafe impl Sendable for char {}
unsafe impl Sendable for () {}

unsafe impl<T: Sendable, const N: usize> Sendable for [T; N] {}
