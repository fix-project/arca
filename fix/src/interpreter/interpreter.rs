extern crate alloc;
use alloc::collections::BTreeMap;

use crate::{
    FixShell, Storage,
    parser::{Expr, Statement},
};
use fixhandle::{Handle, Object};
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

    pub fn interpret_program(&mut self, program: Vec<Statement>) {
        for statement in program {
            match statement {
                Statement::Assign { name, expr } => {
                    let handle = self.interpret(&expr);
                    self.context.insert(name, handle);
                }
                Statement::Print(expr) | Statement::Expr(expr) => {
                    let handle = self.interpret(&expr);
                    println!("handle:    {handle}");

                    if Self::is_blob_obj(handle) {
                        let contents = self.get_blob_data(handle);
                        println!("result is a Blob: {contents:?}");
                        if contents.len() == 8 {
                            let bytes: [u8; 8] = (*contents).try_into().unwrap();
                            let value = u64::from_le_bytes(bytes);
                            println!("\tas a u64: {value}");
                        }
                    }
                }
            }
        }
    }

    pub fn interpret(&self, expression: &Expr) -> Handle {
        match expression {
            Expr::String(str) => self.create_blob(str.as_bytes()),
            Expr::Number(num) => self.create_blob(&i64::to_le_bytes(*num)),
            _ => todo!("support more expressions"),
        }
    }
}

impl FixShell for Interpreter<'_> {
    type Handle = Handle;

    fn create_blob(&self, data: &[u8]) -> Self::Handle {
        self.storage.add_blob(data).into()
    }

    fn create_tree(&self, data: &[Self::Handle]) -> Self::Handle {
        self.storage.add_tree(data).into()
    }

    fn is_blob_obj(handle: Self::Handle) -> bool {
        matches!(handle, Handle::Object(Object::Blob(_)))
    }

    fn is_tree_obj(handle: Self::Handle) -> bool {
        matches!(handle, Handle::Object(Object::Tree(_)))
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
}
