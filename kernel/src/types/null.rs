#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Null;

impl Null {
    pub fn new() -> Self {
        Null
    }
}

impl Default for Null {
    fn default() -> Self {
        Self::new()
    }
}
