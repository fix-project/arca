extern crate alloc;

use super::*;

pub mod arca;

/// A Fix runtime. A Runtime is effectively a data store that can additionally execute procedures.
pub trait Runtime {
    fn storage(&self) -> &dyn Storage;
    /// Requires a Value combination; returns `[[], Ref([Apply, combination, result])]`, or
    /// fails without a coupon when the combination is not an application.
    fn execute(&self, combination: Tree) -> Result<Tree, InvalidApplication>;
}

/// A combination whose callable is not a function: no procedure or continuation can be
/// found for it, or it gives a continuation other than one reply. Such a combination has
/// no application, so nothing certifies a result for it, and an evaluation that needs it
/// fails with the Fix error `request`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidApplication;

impl InvalidApplication {
    /// The `Fix.error` request reporting the failure.
    pub fn request(storage: &dyn Storage) -> Tree {
        storage.add_tree(&[
            storage.add_blob(b"Fix.error").into(),
            storage.add_blob(b"not a function").into(),
        ])
    }
}
