use kernel::prelude::Box;

pub trait FixShell: Sized {
    type Handle;

    fn create_blob(&self, data: &[u8]) -> Self::Handle;
    fn create_tree(&self, data: &[Self::Handle]) -> Self::Handle;
    fn create_ref(handle: Self::Handle) -> Self::Handle;

    fn get_blob_data(&self, handle: Self::Handle) -> Box<[u8]>;
    fn get_tree_data(&self, handle: Self::Handle) -> Box<[Self::Handle]>;

    fn create_application_thunk(handle: Self::Handle) -> Self::Handle;
    fn create_identification_thunk(handle: Self::Handle) -> Self::Handle;

    fn create_strict_encode(handle: Self::Handle) -> Self::Handle;
}
