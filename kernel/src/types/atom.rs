#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct Atom {
    hash: [u8; 32],
}

impl Atom {
    pub fn new<T: AsRef<[u8]>>(x: T) -> Self {
        let data = x.as_ref();
        let hash = blake3::hash(data);
        Atom { hash: hash.into() }
    }
}

impl<T: AsRef<[u8]>> From<T> for Atom {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}
