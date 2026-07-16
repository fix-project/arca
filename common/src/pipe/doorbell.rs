/// Notify the waiter on newly available event (readable/writable)
///
/// ring blocks until it's possible to ring
pub trait DoorBell {
    fn ring(&self);
}
