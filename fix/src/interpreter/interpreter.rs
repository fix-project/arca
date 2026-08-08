extern crate alloc;
use alloc::collections::BTreeMap;

use crate::{FixShell, Storage, parser::Expr};
use fixhandle::*;
use kernel::prelude::*;

pub struct Interpreter<'a> {
    storage: &'a dyn Storage,
    context: BTreeMap<String, Handle>,
}

impl<'a> Interpreter<'a> {
    pub fn new(storage: &'a dyn Storage) -> Self {
        Self {
            storage,
            context: BTreeMap::new(),
        }
    }

    pub fn interpret(&mut self, expression: &Expr) -> Handle {
        match expression {
            Expr::String(str) => self.create_blob(str.as_bytes()),
            Expr::Number(num) => self.create_blob(&i64::to_le_bytes(*num)),
            Expr::Bytes(bytes) => self.create_blob(bytes),
            Expr::Identifier(name) => *self.context.get(name).expect("undefined identifier"),
            Expr::Ref(object) => Self::create_ref(self.interpret(object)),
            Expr::Tree(handles) => {
                let handles: Vec<Handle> = handles.iter().map(|x| self.interpret(x)).collect();
                self.create_tree(&handles)
            }
            Expr::Application(tree) => Self::create_application_thunk(self.interpret(tree)),
            Expr::Identification(tree) => Self::create_identification_thunk(self.interpret(tree)),
            Expr::StrictEncode(thunk) => Self::create_strict_encode(self.interpret(thunk)),
            Expr::Let { bindings, body } => {
                let outer_context = self.context.clone();
                for (name, expr) in bindings {
                    let handle = self.interpret(expr);
                    self.context.insert(name.clone(), handle);
                }
                let handle = self.interpret(body);
                self.context = outer_context;
                handle
            }
        }
    }
}

impl FixShell for Interpreter<'_> {
    type Handle = Handle;

    fn create_blob(&self, data: &[u8]) -> Self::Handle {
        self.storage
            .add_blob(data)
            .expect("storage failed to create blob")
            .into()
    }

    fn create_tree(&self, data: &[Self::Handle]) -> Self::Handle {
        self.storage
            .add_tree(data)
            .expect("storage failed to create tree")
            .into()
    }

    fn create_ref(handle: Self::Handle) -> Self::Handle {
        match handle {
            Handle::Object(Object::Blob(blob)) => Handle::Ref(Ref::Blob(blob)),
            Handle::Object(Object::Tree(tree)) => Handle::Ref(Ref::Tree(tree)),
            _ => panic!("expected blob or tree handle"),
        }
    }

    fn get_blob_data(&self, handle: Self::Handle) -> Box<[u8]> {
        let Handle::Object(Object::Blob(blob)) = handle else {
            panic!("expected blob handle")
        };
        self.storage
            .get_blob(blob)
            .expect("blob data exists for handle")
    }

    fn get_tree_data(&self, handle: Self::Handle) -> Box<[Self::Handle]> {
        let Handle::Object(Object::Tree(tree)) = handle else {
            panic!("expected tree handle")
        };
        self.storage
            .get_tree(tree)
            .expect("tree data exists for handle")
    }

    fn create_application_thunk(handle: Self::Handle) -> Self::Handle {
        let Handle::Object(Object::Tree(tree)) = handle else {
            panic!("expected tree handle for applicaiton")
        };
        Thunk::Application(tree).into()
    }

    fn create_identification_thunk(handle: Self::Handle) -> Self::Handle {
        Thunk::Identification(match handle {
            Handle::Object(Object::Blob(blob)) => Ref::Blob(blob),
            Handle::Object(Object::Tree(tree)) => Ref::Tree(tree),
            Handle::Ref(reference) => reference,
            _ => panic!("expected blob or tree handle"),
        })
        .into()
    }

    fn create_strict_encode(handle: Self::Handle) -> Self::Handle {
        let Handle::Thunk(thunk) = handle else {
            panic!("expected thunk for strict encode")
        };
        Encode::Strict(thunk).into()
    }
}
