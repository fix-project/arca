use super::prelude::*;

impl<R: Runtime> Null<R> {
    pub fn new() -> Self {
        R::create_null()
    }
}

impl<R: Runtime> Default for Null<R> {
    fn default() -> Self {
        Self::new()
    }
}
