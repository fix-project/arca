extern crate alloc;

use alloc::{vec::Vec};

#[derive(Clone)]
pub struct Descriptors<T> {
    table: Vec<Option<T>>,
}

impl<T> Default for Descriptors<T> {
    fn default() -> Self {
        Descriptors { table: Vec::with_capacity(512) }
    }
}

impl<T> Descriptors<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, item: T) -> usize {
        if let Some(i) =
            self.table
                .iter()
                .enumerate()
                .find_map(|(i, x)| if x.is_none() { Some(i) } else { None })
        {
            self.table[i] = Some(item);
            i
        } else {
            let i = self.table.len();
            self.table.push(Some(item));
            i
        }
    }

    pub fn get(&self, i: usize) -> Option<&T> {
        self.table.get(i)?.as_ref()
    }

    pub fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        self.table.get_mut(i)?.as_mut()
    }

    pub fn set(&mut self, i: usize, item: T) -> Option<T> {
        while i >= self.table.len() {
            self.table.push(None);
        }
        self.table[i].replace(item)
    }

    pub fn remove(&mut self, i: usize) -> Option<T> {
        let result = self.table.get_mut(i).and_then(|x| x.take());
        self.shrinkwrap();
        result
    }

    fn shrinkwrap(&mut self) {
        if let Some((last, _)) = self
            .table
            .iter()
            .enumerate()
            .rev()
            .take_while(|(_, x)| x.is_none())
            .last()
        {
            self.table.truncate(last + 1);
        }
    }

    pub fn iter(&'_ self) -> Iter<'_, T> {
        Iter { i: 0, d: self }
    }
}

pub struct Iter<'a, T> {
    i: usize,
    d: &'a Descriptors<T>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (usize, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.i >= self.d.table.len() {
                return None;
            }
            if let Some(x) = self.d.get(self.i) {
                let i = self.i;
                self.i += 1;
                return Some((i, x));
            }
            self.i += 1;
        }
    }
}
