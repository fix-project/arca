use crate::{Root, child, root};
use fixwasm::{Error, Value, coupon::Statement, tree};

#[derive(Clone)]
pub struct Request {
    pub operation: Statement,
    pub input: Root,
    key: (u64, [u8; 32]),
}

impl Request {
    pub fn new(operation: Statement, input: &Root) -> Result<Self, Error> {
        Ok(Self {
            operation,
            input: input.clone(),
            key: (operation as u64, input.raw_handle_bytes()?),
        })
    }

    pub fn value(&self) -> Result<Root, Error> {
        root(tree![self.operation as u64, &self.input])
    }

    pub fn key(&self) -> (u64, [u8; 32]) {
        self.key
    }

    pub fn read(value: &Root) -> Result<Self, Error> {
        let operation = match child(value, 0)?.read_u64()? {
            1 => Statement::Eval,
            2 => Statement::Apply,
            3 => Statement::Force,
            4 => Statement::Think,
            6 => Statement::Select,
            _ => return Err(Error::WrongType),
        };
        Self::new(operation, &child(value, 1)?)
    }
}
