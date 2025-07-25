#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Word {
    value: u64,
}

impl Word {
    pub fn new(value: u64) -> Word {
        Word { value }
    }

    pub fn read(&self) -> u64 {
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

impl AsRef<u64> for Word {
    fn as_ref(&self) -> &u64 {
        &self.value
    }
}

impl AsMut<u64> for Word {
    fn as_mut(&mut self) -> &mut u64 {
        &mut self.value
    }
}
