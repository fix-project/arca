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

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies Word::read returns the value passed to new.
    #[test]
    fn test_read() {
        let word = Word::new(123);
        assert_eq!(word.read(), 123);
    }

    /// Verifies From<u64> and Into<u64> round-trip correctly.
    #[test]
    fn test_from_u64_roundtrip() {
        let word = Word::from(0xdeadbeef_u64);
        assert_eq!(u64::from(word), 0xdeadbeef);
    }

    /// Verifies AsRef and AsMut provide access to the inner value.
    #[test]
    fn test_as_ref_as_mut() {
        let mut word = Word::new(42);
        assert_eq!(*word.as_ref(), 42);
        *word.as_mut() = 99;
        assert_eq!(word.read(), 99);
    }
}
