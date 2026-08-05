use kernel::prelude::Box;

pub trait FixShell: Sized {
    type Handle;

    fn create_blob(&self, data: &[u8]) -> Self::Handle;
    fn create_tree(&self, data: &[Self::Handle]) -> Self::Handle;

    fn is_blob_obj(handle: Self::Handle) -> bool;
    fn is_tree_obj(handle: Self::Handle) -> bool;

    fn get_blob_data(&self, handle: Self::Handle) -> Box<[u8]>;
    fn get_tree_data(&self, handle: Self::Handle) -> Box<[Self::Handle]>;
}
