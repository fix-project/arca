use super::*;

pub(crate) fn status(code: u32) -> Result<(), Error> {
    match code {
        0 => Ok(()),
        1 => Err(Error::GrowFailed),
        2 => Err(Error::OutOfBounds),
        3 => Err(Error::WrongType),
        4 => Err(Error::NotEq),
        6 => Err(Error::AllOccupied),
        _ => unreachable!(),
    }
}
pub(super) fn code(result: Result<(), Error>) -> u32 {
    result.map_or_else(|error| error as u32, |()| 0)
}
pub(super) fn query_current(
    operation: unsafe extern "C" fn(*mut u32) -> u32,
) -> Result<u32, Error> {
    let mut out = 0;
    status(unsafe { operation(&mut out) })?;
    Ok(out)
}
pub fn __finish(output: impl Value) -> Result<(), Error> {
    output.build_at(None)?;
    unsafe { fix_sdk_finish() };
    Ok(())
}
unsafe extern "C" {
    pub(super) fn fix_sdk_blob(bytes: *const u8, length: usize) -> u32;
    pub(super) fn fix_sdk_tree(
        length: usize,
        tag: i32,
        child: unsafe extern "C" fn(*const (), usize) -> u32,
        context: *const (),
    ) -> u32;
    pub(super) fn fix_sdk_reference() -> u32;
    pub(super) fn fix_sdk_identification() -> u32;
    pub(super) fn fix_sdk_application() -> u32;
    pub(super) fn fix_sdk_selection() -> u32;
    pub(super) fn fix_sdk_strict() -> u32;
    pub(super) fn fix_sdk_shallow() -> u32;
    pub(super) fn fix_sdk_entry(index: usize) -> u32;
    pub(super) fn fix_sdk_value_type(out: *mut u32) -> u32;
    pub(super) fn fix_sdk_data_type(out: *mut u32) -> u32;
    pub(super) fn fix_sdk_encode_type(out: *mut u32) -> u32;
    pub(super) fn fix_sdk_len(out: *mut u32) -> u32;
    pub(super) fn fix_sdk_is_tag(out: *mut u32) -> u32;
    pub(super) fn fix_sdk_is_eq(out: *mut u32) -> u32;
    pub(super) fn fix_sdk_read(offset: usize, destination: *mut u8, length: usize) -> u32;
    pub(super) fn fix_sdk_equals(
        rhs: unsafe extern "C" fn(*const ()) -> u32,
        context: *const (),
        out: *mut i32,
    ) -> u32;
    pub(super) fn fix_sdk_root();
    pub(super) fn fix_sdk_finish();
}

pub(super) fn current_value_type() -> Result<ValueType, Error> {
    Ok(match query_current(fix_sdk_value_type)? {
        0 => ValueType::Object,
        1 => ValueType::Ref,
        2 => ValueType::Thunk,
        3 => ValueType::Encode,
        _ => unreachable!(),
    })
}
pub(super) fn current_data_type() -> Result<DataType, Error> {
    Ok(match query_current(fix_sdk_data_type)? {
        0 => DataType::Blob,
        1 => DataType::Tree,
        _ => unreachable!(),
    })
}
pub(super) fn current_encode_type() -> Result<EncodeType, Error> {
    Ok(match query_current(fix_sdk_encode_type)? {
        0 => EncodeType::Strict,
        1 => EncodeType::Shallow,
        _ => unreachable!(),
    })
}
