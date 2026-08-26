use kernel::prelude::Box;

pub trait FixShell: Sized {
    type Handle;

    fn create_blob(&self, data: &[u8]) -> Self::Handle;
    fn create_tree(&self, data: &[Self::Handle]) -> Self::Handle;
    fn create_ref(handle: Self::Handle) -> Self::Handle;

    #[allow(dead_code)] // these functions are unused so far
    // (and ultimately the interpreter should use the "real" Fix shell)
    fn get_blob_data(&self, handle: Self::Handle) -> Box<[u8]>;
    #[allow(dead_code)]
    fn get_tree_data(&self, handle: Self::Handle) -> Box<[Self::Handle]>;

    fn create_application_thunk(handle: Self::Handle) -> Self::Handle;
    fn create_identification_thunk(handle: Self::Handle) -> Self::Handle;

    fn create_strict_encode(handle: Self::Handle) -> Self::Handle;
}
