use crate::{Error, value::status};
pub(crate) struct Resources;
impl Resources {
    pub(crate) fn new() -> Result<Self, Error> {
        status(unsafe { fix_sdk_begin() })?;
        Ok(Self)
    }
}
impl Drop for Resources {
    fn drop(&mut self) {
        unsafe { fix_sdk_reset() }
    }
}
unsafe extern "C" {
    fn fix_sdk_begin() -> u32;
    fn fix_sdk_reset();
}
