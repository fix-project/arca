use core::sync::atomic::{AtomicUsize, Ordering};

#[core_local]
pub(crate) static mut ACPI_ID: u32 = 0;

#[core_local]
pub(crate) static mut IS_BOOTSTRAP: bool = false;

pub(crate) static NCORES: AtomicUsize = AtomicUsize::new(0);

pub fn id() -> u32 {
    unsafe { *ACPI_ID }
}

pub fn is_bootstrap() -> bool {
    unsafe { *IS_BOOTSTRAP }
}

pub fn ncores() -> usize {
    NCORES.load(Ordering::Acquire)
}
