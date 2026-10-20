use crate::handle::*;
use crate::runtime::Runtime;
use crate::storage::Storage;
use kernel::prelude::*;

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
        let coupon = self.runtime.execute(combination);
        let claim = self.storage().certificate_claim(coupon).unwrap();
        self.storage().get_tree(claim).unwrap()[2]
    }

    pub fn select(&self, selection: Tree) -> Handle {
        let selection = self.eval_tree(selection);
        self.storage().select(selection).expect("invalid selection")
    }

    pub fn lift(&self, handle: Handle) -> Handle {
        match handle.view() {
            HandleView::Ref(reference) => reference.object_descriptor().into(),
            _ => handle,
        }
    }

    pub fn lower(&self, handle: Handle) -> Handle {
        match handle.view() {
            HandleView::Object(object) => object.into_ref().into(),
            _ => handle,
        }
    }

    pub fn think(&self, thunk: Thunk) -> Handle {
        match thunk.view() {
            ThunkView::Identification(reference) => reference.into(),
            ThunkView::Selection(tree) => self.select(tree.object_descriptor()),
            ThunkView::Application(tree) => {
                let evaled = self.eval_tree(tree.object_descriptor());
                self.apply(evaled)
            }
        }
    }

    fn force(&self, thunk: Thunk) -> Handle {
        let thought = self.think(thunk);
        match thought.view() {
            HandleView::Object(_) | HandleView::Ref(_) => thought,
            HandleView::Thunk(thunk) => self.force(*thunk),
            HandleView::Encode(encode) => match encode.view() {
                EncodeView::Strict(thunk) | EncodeView::Shallow(thunk) => self.force(thunk),
            },
        }
    }

    fn encode(&self, encode: Encode) -> Handle {
        match encode.view() {
            EncodeView::Strict(thunk) => self.lift(self.force(thunk)),
            EncodeView::Shallow(thunk) => self.lower(self.force(thunk)),
        }
    }

    fn eval_tree(&self, handle: Tree) -> Tree {
        if handle.as_handle().is_value() {
            return handle;
        }
        let tree = self.runtime.storage().get_tree(handle).unwrap();
        let evaled: Vec<Handle> = tree
            .as_ref()
            .iter()
            .copied()
            .map(|x| self.eval(x))
            .collect();
        if tree
            .iter()
            .zip(&evaled)
            .all(|(a, b)| a.as_bytes() == b.as_bytes())
        {
            handle
        } else {
            let result = self.runtime.storage().add_tree(&evaled);
            if handle.is_tag() {
                result.tag_descriptor()
            } else {
                result
            }
        }
    }

    pub fn eval(&self, handle: Handle) -> Handle {
        #[cfg(debug_assertions)]
        println!("evaluating {handle}");
        match handle.view() {
            HandleView::Ref(_) | HandleView::Thunk(_) => handle,
            HandleView::Object(obj) => match obj.view() {
                ObjectView::Blob(blob) => (*blob).into(),
                ObjectView::Tree(tree) => self.eval_tree(*tree).into(),
            },
            HandleView::Encode(e) => self.eval(self.encode(*e)),
        }
    }
}
