use crate::{Root, child, root};
use fix::{Error, Value, coupon::Statement, tree};

#[derive(Clone)]
pub struct Request {
    pub statement: Statement,
    pub input: Root,
    key: (u64, [u8; 32]),
}

impl Request {
    pub fn new(statement: Statement, input: &Root) -> Result<Self, Error> {
        Ok(Self {
            statement,
            input: input.clone(),
            key: (statement as u64, input.raw_handle_bytes()?),
        })
    }

    pub fn value(&self) -> Result<Root, Error> {
        root(tree![self.statement as u64, &self.input])
    }

    pub fn key(&self) -> (u64, [u8; 32]) {
        self.key
    }

    pub fn read(value: &Root) -> Result<Self, Error> {
        let statement = match child(value, 0)?.read_u64()? {
            1 => Statement::Eval,
            2 => Statement::Apply,
            3 => Statement::Force,
            4 => Statement::Think,
            6 => Statement::Select,
            _ => return Err(Error::WrongType),
        };
        Self::new(statement, &child(value, 1)?)
    }
}
