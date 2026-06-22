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

    fn lift(&self, handle: Handle) -> Handle {
        match handle {
            Handle::Ref(r) => match r {
                Ref::Tree(t) => Object::Tree(t).into(),
                Ref::Blob(b) => Object::Blob(b).into(),
            },
            _ => handle,
        }
    }

    fn lower(&self, handle: Handle) -> Handle {
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
            Thunk::Identification(_) => todo!(),
            Thunk::Selection(_) => todo!(),
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
            Handle::Thunk(_) | Handle::Encode(_) => todo!(),
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
            Handle::Thunk(_) | Handle::Ref(_) => todo!(),
            Handle::Object(obj) => match obj {
                Object::Blob(x) => x.into(),
                Object::Tree(tree) => self.eval_tree(tree).into(),
            },
            Handle::Encode(e) => self.eval(self.encode(e)),
        }
    }
}
