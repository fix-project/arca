use common::message::Handle;

use crate::prelude::*;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Word {
    value: u64,
}

impl Word {
    pub fn new(value: u64) -> Word {
        Word { value }
    }
}

impl arca::RuntimeType for Word {
    type Runtime = Runtime;
}

impl arca::ValueType for Word {
    const DATATYPE: DataType = DataType::Word;
}

impl arca::Word for Word {
    fn read(&self) -> u64 {
        self.value
    }
}

impl From<u64> for Word {
    fn from(value: u64) -> Self {
        Word::new(value)
    }
}

impl From<Word> for u64 {
    fn from(value: Word) -> Self {
        value.read()
    }
}

impl TryFrom<Handle> for Word {
    type Error = Handle;

    fn try_from(value: Handle) -> Result<Self, Self::Error> {
        if value.datatype() == <Self as arca::ValueType>::DATATYPE {
            Ok(Word::new(value.read().0 as u64))
        } else {
            Err(value)
        }
    }
}

impl From<Word> for Handle {
    fn from(value: Word) -> Self {
        Handle::new(DataType::Word, (value.read() as usize, 0))
    }
}
