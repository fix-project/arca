use crate::{Handle, HandleView, Name, Object, ObjectView, Tree};

/// Visit backing Objects in post-order using ordinary recursion.
/// Refs, Thunks, and Encodes are opaque unless `deferred` is enabled.
/// Tag author identities and inline Blobs require no backing. Shared Objects may be visited repeatedly.
pub fn visit<I, E>(
    root: Handle,
    deferred: bool,
    read: &mut impl FnMut(Tree) -> Result<I, E>,
    visitor: &mut impl FnMut(Object) -> Result<(), E>,
) -> Result<(), E>
where
    I: IntoIterator<Item = Handle>,
{
    let object = match root.view() {
        HandleView::Object(object) => *object,
        _ if deferred => root.object_descriptor(),
        _ => return Ok(()),
    };
    if matches!(object.as_handle().name(), Name::Literal(_)) {
        return Ok(());
    }
    if let ObjectView::Tree(tree) = object.view() {
        for (index, child) in read(*tree)?.into_iter().enumerate() {
            if !tree.is_tag() || index != 0 {
                visit(child, deferred, read, visitor)?;
            }
        }
    }
    visitor(object)
}
