use crate::cls;
use core::sync::atomic::{AtomicUsize, Ordering};

pub(crate) static NCORES: AtomicUsize = AtomicUsize::new(0);

pub fn id() -> usize {
    cls::CLS.acpi_id
}

pub fn is_bootstrap() -> bool {
    cls::CLS.bootstrap
}

pub fn ncores() -> usize {
    NCORES.load(Ordering::Acquire)
}
