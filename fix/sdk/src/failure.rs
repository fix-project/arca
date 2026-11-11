use core::fmt::{self, Write};

struct Message {
    length: u32,
}

impl Write for Message {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let count = u32::try_from(text.len()).map_err(|_| fmt::Error)?;
        let end = self.length.checked_add(count).ok_or(fmt::Error)?;
        let pages = end.div_ceil(65536);
        unsafe {
            let current = memory_size_1();
            if pages > current && memory_grow_1(pages - current) == u32::MAX {
                return Err(fmt::Error);
            }
            memory_copy_0_to_1(self.length, text.as_ptr() as u32, count);
        }
        self.length = end;
        Ok(())
    }
}

pub(crate) fn report(message: &impl fmt::Display) -> ! {
    let mut buffer = Message { length: 0 };
    if write!(&mut buffer, "{message}").is_err() {
        core::arch::wasm32::unreachable();
    }
    let message: crate::Handle =
        unsafe { crate::Handle::from_raw(crate::ffi::create_blob(1, buffer.length)) };
    let message = fix::Value::<crate::backend::Shell>::from_inner(message);
    let marker = crate::backend::Blob::from("Rust.panic");
    crate::exit(crate::backend::Tree::new(&[marker.into(), message]))
}

#[cfg(feature = "std")]
pub(crate) fn install_hook() {
    std::panic::set_hook(alloc::boxed::Box::new(|info| {
        if let Some(message) = info.payload().downcast_ref::<&str>() {
            report(message);
        }
        if let Some(message) = info.payload().downcast_ref::<alloc::string::String>() {
            report(message);
        }
        report(&"non-string panic payload");
    }));
}

#[link(wasm_import_module = "__wasm_postprocessor")]
unsafe extern "C" {
    fn memory_size_1() -> u32;
    fn memory_grow_1(pages: u32) -> u32;
    fn memory_copy_0_to_1(destination: u32, source: u32, length: u32);
}
