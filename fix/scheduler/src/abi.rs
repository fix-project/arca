use crate::shell::{Value, capture};
use fixwasm::{Blob, Error, Handle, Object, Tree, Value as AsValue};

pub(crate) fn request(value: impl AsValue) -> Result<(u64, Value), Error> {
    let reply = Handle::<Object<Tree>>::try_from(value.effect()?)?;
    Ok((reply.get(0).read_u64()?, capture(reply.get(1))?))
}
pub(crate) fn coordinate(name: &[u8], arguments: &[Value]) -> Result<(u64, Value), Error> {
    let arguments = arguments
        .iter()
        .map(|value| value.inner().as_raw())
        .collect::<alloc::vec::Vec<_>>();
    let mut status = 0u64;
    let value = unsafe {
        Handle::from_raw(fixwasm::ffi::coordinate_handles(
            name.as_ptr() as u32,
            name.len().try_into()?,
            arguments.as_ptr() as u32,
            arguments.len().try_into()?,
            &mut status as *mut _ as u32,
        ))
    };
    Ok((status, Value::from_inner(value)))
}
pub(crate) fn coordinated(name: &[u8], arguments: &[Value]) -> Result<Value, Error> {
    let (status, value) = coordinate(name, arguments)?;
    assert_eq!(status, 0, "scheduler coordination failed");
    Ok(value)
}
pub(crate) fn effect(name: &[u8], arguments: &[Value]) -> Result<Value, Error> {
    let name = capture(Blob::create(name))?;
    let values = core::iter::once(&name).chain(arguments);
    let (status, value) = request(Tree::from_iter(values))?;
    assert_eq!(status, 0, "scheduler effect failed");
    Ok(value)
}
