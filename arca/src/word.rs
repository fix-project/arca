use super::prelude::*;

impl<R: Runtime> Word<R> {
    pub fn new(word: u64) -> Self {
        R::create_word(word)
    }

    pub fn read(&self) -> u64 {
        R::read_word(self)
    }
}

impl<R: Runtime> From<u64> for Word<R> {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}
