use super::prelude::*;

impl<R: Runtime> Table<R> {
    pub fn new(len: usize) -> Self {
        R::create_table(len)
    }

    pub fn get(&self, idx: usize) -> Result<Entry<R>, R::Error> {
        R::get_table(self, idx)
    }

    pub fn set(&mut self, idx: usize, entry: Entry<R>) -> Result<Entry<R>, R::Error> {
        R::set_table(self, idx, entry)
    }

    pub fn take(&mut self, idx: usize) -> Result<Entry<R>, R::Error> {
        let replacement = Entry::default();
        self.set(idx, replacement)
    }

    pub fn swap(&mut self, idx: usize, value: &mut Entry<R>) -> Result<(), R::Error> {
        let mut replacement = self.take(idx)?;
        core::mem::swap(&mut replacement, value);
        self.set(idx, replacement)?;
        Ok(())
    }

    pub fn map(&mut self, address: usize, entry: Entry<R>) -> Result<Entry<R>, R::Error> {
        let result = if address + entry.len() >= self.len() {
            try_replace_with(self, |this: Self| -> Result<Self, R::Error> {
                let mut embiggened = R::create_table(this.len() * 512);
                embiggened.set(0, Entry::RWTable(this))?;
                Ok(embiggened)
            })?;
            self.map(address, entry)?
        } else if entry.len() == self.len() / 512 {
            let shift = entry.len().ilog2();
            let index = address >> shift;
            assert!(index < 512);
            self.set(index, entry)?
        } else {
            let shift = (self.len() / 512).ilog2();
            let index = (address >> shift) & 0x1ff;
            let offset = address & !(0x1ff << shift);

            let mut smaller = match self.set(index, Entry::Null(0))? {
                Entry::ROTable(table) => table,
                Entry::RWTable(table) => table,
                _ => R::create_table(self.len() / 512),
            };
            assert!(self.len() > smaller.len());
            smaller.map(offset, entry)?;
            self.set(index, Entry::RWTable(smaller))?
        };
        Ok(result)
    }

    pub fn unmap(&mut self, address: usize) -> Option<Entry<R>> {
        if address > self.len() {
            None
        } else {
            let shift = (self.len() / 512).ilog2();
            let index = (address >> shift) & 0x1ff;
            let offset = address & !(0x1ff << shift);

            let smaller = self.set(index, Entry::Null(0));
            match smaller.ok()? {
                Entry::ROTable(mut table) => {
                    let nested = table.unmap(offset);
                    assert!(self.set(index, Entry::ROTable(table)).is_ok());
                    nested
                }
                Entry::RWTable(mut table) => {
                    let nested = table.unmap(offset);
                    assert!(self.set(index, Entry::RWTable(table)).is_ok());
                    nested
                }
                x => Some(x),
            }
        }
    }

    pub fn iter(&self) -> TableIter<'_, R> {
        TableIter {
            table: self,
            len: 512,
            index: 0,
        }
    }

    #[cfg(feature = "alloc")]
    pub fn with_ref<T>(&self, f: impl FnOnce(&[Entry<R>]) -> T) -> T {
        R::with_table_as_ref(self, f)
    }
}

fn try_replace_with<T, E>(x: &mut T, f: impl FnOnce(T) -> Result<T, E>) -> Result<(), E> {
    unsafe {
        let old = core::ptr::read(x);
        let new = f(old)?;
        core::ptr::write(x, new);
        Ok(())
    }
}

pub struct TableIter<'a, R: Runtime> {
    table: &'a Table<R>,
    len: usize,
    index: usize,
}

impl<'a, R: Runtime> Iterator for TableIter<'a, R> {
    type Item = Entry<R>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        let next = self.table.get(self.index);
        self.index += 1;
        Some(next.unwrap())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len - self.index, Some(self.len - self.index))
    }
}

impl<R: Runtime> ExactSizeIterator for TableIter<'_, R> {}

#[derive(Debug, Clone)]
pub struct TableIntoIter<R: Runtime> {
    table: Table<R>,
    len: usize,
    index: usize,
}

impl<R: Runtime> IntoIterator for Table<R> {
    type Item = Entry<R>;

    type IntoIter = TableIntoIter<R>;

    fn into_iter(self) -> Self::IntoIter {
        let len = self.len();
        TableIntoIter {
            table: self,
            len,
            index: 0,
        }
    }
}

impl<R: Runtime> Iterator for TableIntoIter<R> {
    type Item = Entry<R>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        let next = self
            .table
            .take(self.index)
            .expect("could not read elements of table");
        self.index += 1;
        Some(next)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len - self.index, Some(self.len - self.index))
    }
}

impl<R: Runtime> ExactSizeIterator for TableIntoIter<R> {}
