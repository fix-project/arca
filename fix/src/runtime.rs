pub trait DeterministicEquivRuntime {
    type BlobData: Clone + core::fmt::Debug;
    type TreeData: Clone + core::fmt::Debug;
    type Handle: Clone + core::fmt::Debug;
    type Error;

    fn create_blob_i64(data: u64) -> Self::Handle;
    fn create_blob(data: Self::BlobData) -> Self::Handle;
    fn create_tree(data: Self::TreeData) -> Self::Handle;

    fn length(handle: &Self::Handle) -> Result<usize, Self::Error>;

    fn get_blob(handle: &Self::Handle) -> Result<&[u8], Self::Error>;
    fn get_tree_entry(data: &Self::TreeData, index: usize) -> Result<Self::Handle, Self::Error>;

    fn set_tree_entry(
        data: &mut Self::TreeData,
        index: usize,
        handle: &Self::Handle,
    ) -> Result<Self::Handle, Self::Error>;

    fn is_blob(handle: &mut Self::Handle) -> bool;
    fn is_tree(handle: &mut Self::Handle) -> bool;
}

pub trait ExecutionRuntime: DeterministicEquivRuntime {
    fn execute(combination: &Self::Handle) -> Result<Self::Handle, Self::Error>;
}
