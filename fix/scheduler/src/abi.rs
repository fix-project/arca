use crate::shell::{Root, root};
use fixwasm::{Blob, Error, Handle, Object, Tree, Value};

pub(crate) fn request(value: impl Value) -> Result<(u64, Root), Error> {
    let reply = Handle::<Object<Tree>>::try_from(value.effect()?)?;
    Ok((reply.get(0).read_u64()?, root(reply.get(1))?))
}
pub(crate) fn coordinate(name: &[u8], arguments: &[Root]) -> Result<(u64, Root), Error> {
    let (status, value) = Tree::from_iter(arguments).coordinate(name)?;
    Ok((status, alloc::rc::Rc::new(value)))
}
pub(crate) fn coordinated(name: &[u8], arguments: &[Root]) -> Result<Root, Error> {
    let (status, value) = coordinate(name, arguments)?;
    assert_eq!(status, 0, "scheduler coordination failed");
    Ok(value)
}
pub(crate) fn effect(name: &[u8], arguments: &[Root]) -> Result<Root, Error> {
    let name = root(Blob::create(name))?;
    let values = core::iter::once(&name).chain(arguments);
    let (status, value) = request(Tree::from_iter(values))?;
    assert_eq!(status, 0, "scheduler effect failed");
    Ok(value)
}
