use super::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Entry<R: Runtime> {
    Null(usize),
    ROPage(Page<R>),
    RWPage(Page<R>),
    ROTable(Table<R>),
    RWTable(Table<R>),
}

impl<R: Runtime> Default for Entry<R> {
    fn default() -> Self {
        Self::Null(0)
    }
}

impl<R: Runtime> Entry<R> {
    pub fn len(&self) -> usize {
        match self {
            Entry::Null(size) => *size,
            Entry::ROPage(page) => page.len(),
            Entry::RWPage(page) => page.len(),
            Entry::ROTable(table) => table.len(),
            Entry::RWTable(table) => table.len(),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Entry::Null(_))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn byte_size(&self) -> usize {
        match self {
            Entry::Null(_) => 0,
            Entry::ROPage(page) => page.len(),
            Entry::RWPage(page) => page.len(),
            Entry::ROTable(table) => ValueRef::Table(table).byte_size(),
            Entry::RWTable(table) => ValueRef::Table(table).byte_size(),
        }
    }
}
