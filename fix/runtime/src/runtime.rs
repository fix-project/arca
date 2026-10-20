extern crate alloc;

use super::*;

pub mod arca;

/// A Fix runtime. A Runtime is effectively a data store that can additionally execute procedures.
pub trait Runtime {
    fn storage(&self) -> &dyn Storage;
    /// Requires a Value combination; returns `[[], Ref([Apply, combination, result])]`.
    fn execute(&self, combination: Tree) -> Tree;
}
