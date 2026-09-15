use crate::handle::*;
use crate::runtime::Runtime;
use crate::storage::Storage;
use kernel::prelude::*;

// use fixhandle::rawhandle::{Encode, Handle, Object, Ref, Thunk, TreeName};

// use fixruntime::{
//     common::CouponTrades,
//     fixruntime::{FixRuntime, FixTreeData},
//     runtime::{DeterministicEquivRuntime, Executor},
//     storage::FixData,
// };

// use common::bitpack::BitPack;
// use kernel::prelude::*;

pub struct Evaluator<R: Runtime> {
    runtime: R,
}

impl<R: Runtime> Evaluator<R> {
    pub fn new(runtime: R) -> Self {
        Self { runtime }
    }

    pub fn runtime(&self) -> &R {
        &self.runtime
    }

    pub fn storage(&self) -> &dyn Storage {
        self.runtime.storage()
    }

    fn apply(&self, combination: Tree) -> Handle {
        self.runtime.execute(combination)
    }

    pub fn select(&self, selection: Tree) -> Handle {
        let handles = self.storage().get_tree(selection).unwrap();
        match *handles {
            [target, index] => self.select_index(target, self.read_index(index)),
            [target, start, end] => {
                self.select_range(target, self.read_index(start), self.read_index(end))
            }
            _ => panic!("selection thunk got {} handles", handles.len()),
        }
    }

    pub fn select_index(&self, target: Handle, index: usize) -> Handle {
        if index >= target.len() {
            panic!("Invalid index {index} for selection thunk");
        }
        match target {
            Handle::Object(Object::Tree(tree)) | Handle::Ref(Ref::Tree(tree)) => {
                self.storage().get_tree(tree).unwrap()[index]
            }
            Handle::Object(Object::Blob(blob)) | Handle::Ref(Ref::Blob(blob)) => {
                let data = self.storage().get_blob(blob).unwrap();
                Ref::Blob(self.storage().add_blob(&[data[index]])).into()
            }
            _ => panic!("expected blob or tree handle for selection thunk"),
        }
    }

    pub fn select_range(&self, target: Handle, begin: usize, end: usize) -> Handle {
        if begin >= end {
            panic!("Invalid range [{begin}, {end}) for seleciton thunk");
        }
        match target {
            Handle::Object(Object::Tree(tree)) | Handle::Ref(Ref::Tree(tree)) => {
                let data = self.storage().get_tree(tree).unwrap();
                Ref::Tree(self.storage().add_tree(&data[begin..end])).into()
            }
            Handle::Object(Object::Blob(blob)) | Handle::Ref(Ref::Blob(blob)) => {
                let data = self.storage().get_blob(blob).unwrap();
                Ref::Blob(self.storage().add_blob(&data[begin..end])).into()
            }
            _ => panic!("expected blob or tree handle for selection thunk"),
        }
    }

    pub fn read_index(&self, handle: Handle) -> usize {
        let Handle::Object(Object::Blob(blob)) = handle else {
            panic!("expected blob handle for selection index")
        };
        let bytes = self.storage().get_blob(blob).unwrap();
        // Make buffer fit all supported integer widths
        let mut buffer = [0; 16];
        assert!(bytes.len() <= buffer.len());
        buffer[..bytes.len()].copy_from_slice(&bytes);
        usize::try_from(u128::from_le_bytes(buffer)).expect("selection index should be in range")
    }

    pub fn lift(&self, handle: Handle) -> Handle {
        match handle {
            Handle::Ref(r) => match r {
                Ref::Tree(t) => Object::Tree(t).into(),
                Ref::Blob(b) => Object::Blob(b).into(),
            },
            _ => handle,
        }
    }

    pub fn lower(&self, handle: Handle) -> Handle {
        match handle {
            Handle::Object(r) => match r {
                Object::Tree(t) => Ref::Tree(t).into(),
                Object::Blob(b) => Ref::Blob(b).into(),
            },
            _ => handle,
        }
    }

    fn think(&self, thunk: Thunk) -> Handle {
        match thunk {
            Thunk::Identification(reference) => self.lift(Handle::Ref(reference)),
            Thunk::Selection(tree) => {
                let evaled = self.eval_tree(tree);
                self.select(evaled)
            }
            Thunk::Application(tree) => {
                let evaled = self.eval_tree(tree);
                self.apply(evaled)
            }
        }
    }

    fn force(&self, thunk: Thunk) -> Handle {
        let thought = self.think(thunk);
        match thought {
            Handle::Object(_) => thought,
            Handle::Ref(_) => self.lift(thought),
            Handle::Thunk(thunk) => self.force(thunk),
            Handle::Encode(encode) => self.lift(self.encode(encode)),
        }
    }

    fn encode(&self, encode: Encode) -> Handle {
        match encode {
            Encode::Strict(thunk) => self.lift(self.force(thunk)),
            Encode::Shallow(thunk) => self.lower(self.force(thunk)),
        }
    }

    fn eval_tree(&self, handle: Tree) -> Tree {
        let tree = self.runtime.storage().get_tree(handle).unwrap();
        let evaled: Vec<Handle> = tree
            .as_ref()
            .iter()
            .copied()
            .map(|x| self.eval(x))
            .collect();
        self.runtime.storage().add_tree(&evaled)
    }

    pub fn eval(&self, handle: Handle) -> Handle {
        println!("evaluating {handle}");
        match handle {
            Handle::Ref(reference) => self.eval(self.lift(Handle::Ref(reference))),
            Handle::Thunk(_) => handle,
            Handle::Object(obj) => match obj {
                Object::Blob(blob) => blob.into(),
                Object::Tree(tree) => self.eval_tree(tree).into(),
            },
            Handle::Encode(e) => self.eval(self.encode(e)),
        }
    }
}
