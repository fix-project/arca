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
        let object = match target.view() {
            HandleView::Object(object) => *object,
            HandleView::Ref(reference) => reference.object_descriptor(),
            _ => panic!("expected blob or tree handle for selection thunk"),
        };
        match object.view() {
            ObjectView::Tree(tree) => self.storage().get_tree(*tree).unwrap()[index],
            ObjectView::Blob(blob) => {
                let data = self.storage().get_blob(*blob).unwrap();
                self.storage().add_blob(&[data[index]]).into_ref().into()
            }
        }
    }

    pub fn select_range(&self, target: Handle, begin: usize, end: usize) -> Handle {
        if begin >= end {
            panic!("Invalid range [{begin}, {end}) for seleciton thunk");
        }
        let object = match target.view() {
            HandleView::Object(object) => *object,
            HandleView::Ref(reference) => reference.object_descriptor(),
            _ => panic!("expected blob or tree handle for selection thunk"),
        };
        match object.view() {
            ObjectView::Tree(tree) => {
                let data = self.storage().get_tree(*tree).unwrap();
                self.storage().add_tree(&data[begin..end]).into_ref().into()
            }
            ObjectView::Blob(blob) => {
                let data = self.storage().get_blob(*blob).unwrap();
                self.storage().add_blob(&data[begin..end]).into_ref().into()
            }
        }
    }

    pub fn read_index(&self, handle: Handle) -> usize {
        let blob =
            fixhandle::Blob::try_from(handle).expect("expected blob handle for selection index");
        let bytes = self.storage().get_blob(blob).unwrap();
        // Make buffer fit all supported integer widths
        let mut buffer = [0; 16];
        assert!(bytes.len() <= buffer.len());
        buffer[..bytes.len()].copy_from_slice(&bytes);
        usize::try_from(u128::from_le_bytes(buffer)).expect("selection index should be in range")
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

    fn think(&self, thunk: Thunk) -> Handle {
        match thunk.view() {
            ThunkView::Identification(reference) => self.lift(reference.into()),
            ThunkView::Selection(tree) => {
                let evaled = self.eval_tree(tree.object_descriptor());
                self.select(evaled)
            }
            ThunkView::Application(tree) => {
                let evaled = self.eval_tree(tree.object_descriptor());
                self.apply(evaled)
            }
        }
    }

    fn force(&self, thunk: Thunk) -> Handle {
        let thought = self.think(thunk);
        match thought.view() {
            HandleView::Object(_) => thought,
            HandleView::Ref(_) => self.lift(thought),
            HandleView::Thunk(thunk) => self.force(*thunk),
            HandleView::Encode(encode) => self.lift(self.encode(*encode)),
        }
    }

    fn encode(&self, encode: Encode) -> Handle {
        match encode.view() {
            EncodeView::Strict(thunk) => self.lift(self.force(thunk)),
            EncodeView::Shallow(thunk) => self.lower(self.force(thunk)),
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
        match handle.view() {
            HandleView::Ref(reference) => self.eval(self.lift((*reference).into())),
            HandleView::Thunk(_) => handle,
            HandleView::Object(obj) => match obj.view() {
                ObjectView::Blob(blob) => (*blob).into(),
                ObjectView::Tree(tree) => self.eval_tree(*tree).into(),
            },
            HandleView::Encode(e) => self.eval(self.encode(*e)),
        }
    }
}
