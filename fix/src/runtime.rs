extern crate alloc;

use super::*;

pub mod arca;

/// A Fix runtime. A Runtime is effectively a data store that can additionally execute procedures.
pub trait Runtime {
    fn storage(&self) -> &dyn Storage;
    fn execute(&self, combination: Tree) -> Handle;
}
