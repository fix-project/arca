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

#[cfg(test)]
mod tests {
    use super::*;

    /// Ensures Null::new() and Null::default() produce identical values.
    #[test]
    fn test_new_equals_default() {
        assert_eq!(Null::new(), Null::default());
    }
}
