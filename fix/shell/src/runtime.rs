use core::clone::Clone;
use core::result::Result;

use fixhandle::rawhandle::FixHandle;

pub trait DeterministicEquivRuntime {
    type BlobData: Clone + core::fmt::Debug;
    type TreeData: Clone + core::fmt::Debug;
    type Handle: Clone + core::fmt::Debug;
    type Error;

    fn create_blob_i64(data: u64) -> Self::Handle;
    fn create_blob(data: Self::BlobData) -> Self::Handle;
    fn create_tree(data: Self::TreeData) -> Self::Handle;

    fn get_blob(handle: Self::Handle) -> Result<Self::BlobData, Self::Error>;
    fn get_tree(handle: Self::Handle) -> Result<Self::TreeData, Self::Error>;

    fn is_blob(handle: Self::Handle) -> bool;
    fn is_tree(handle: Self::Handle) -> bool;
    fn len(handle: Self::Handle) -> usize;
}
