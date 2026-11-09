use super::*;

pub struct Capture {
    index: usize,
}

pub(super) fn from_index(index: usize) -> Handle<Any, Capture> {
    Handle::new(Capture { index })
}

impl Lookup for Capture {
    fn load(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        status(unsafe { fix_sdk_capture(self.index) })?;
        handle::follow(path)
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        unsafe { fix_sdk_release_capture(self.index) };
    }
}

pub(super) fn evaluate<V: Value>(value: &V) -> Result<Handle<Any, Capture>, Error> {
    capture(value, fix_sdk_eval)
}

pub(super) fn capture<V: Value>(
    value: &V,
    operation: unsafe extern "C" fn(*mut usize) -> u32,
) -> Result<Handle<Any, Capture>, Error> {
    value.build_at(None)?;
    let index = query_current(operation)?;
    Ok(Handle::new(Capture { index }))
}
