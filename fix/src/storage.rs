extern crate alloc;

use super::*;
use core::option::Option;
use alloc::boxed::Box;

pub mod memory;

/// An object store, capable of saving and retrieving Fix objects.
pub trait Storage {
    fn add_blob(&self, data: &[u8]) -> Blob;
    fn add_tree(&self, data: &[Handle]) -> Tree;

    fn get_blob(&self, name: Blob) -> Option<Box<[u8]>>;
    fn get_tree(&self, name: Tree) -> Option<Box<[Handle]>>;

    fn has_blob(&self, name: Blob) -> bool {
        self.get_blob(name).is_some()
    }

    fn has_tree(&self, name: Tree) -> bool {
        self.get_tree(name).is_some()
    }
}
