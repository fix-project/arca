use core::clone::Clone;
use core::result::Result;

pub trait MockRuntime {
    type BlobData: Clone + core::fmt::Debug;
    type TreeData: Clone + core::fmt::Debug;
    type Handle: Clone + core::fmt::Debug;
    type Error;

    fn create_blob(&mut self, data: Self::BlobData) -> Self::Handle;
    fn create_tree(&mut self, data: Self::TreeData) -> Self::Handle;

    fn get_blob(&self, handle: &Self::Handle) -> Result<Self::BlobData, Self::Error>;
    fn get_tree(&self, handle: &Self::Handle) -> Result<Self::TreeData, Self::Error>;

    fn apply(&mut self, combination: &Self::Handle) -> Result<Self::Handle, Self::Error>;
}
