#![allow(unused)]
use core::{
    mem::MaybeUninit,
    ops::{Index, IndexMut},
};

#[derive(Debug)]
pub struct ArrayVec<T, const N: usize> {
    data: [MaybeUninit<T>; N],
    size: usize,
}

impl<T, const N: usize> ArrayVec<T, N> {
    pub const fn new() -> Self {
        Self {
            data: unsafe { MaybeUninit::uninit().assume_init() },
            size: 0,
        }
    }

    pub fn push(&mut self, x: T) -> Result<(), T> {
        if self.is_full() {
            return Err(x);
        }
        self.data[self.size] = MaybeUninit::new(x);
        self.size += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        unsafe {
            if self.is_empty() {
                return None;
            }
            self.size -= 1;
            Some(self.data[self.size].assume_init_read())
        }
    }

    pub const fn len(&self) -> usize {
        self.size
    }

    pub const fn capacity(&self) -> usize {
        N
    }

    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub const fn is_full(&self) -> bool {
        self.len() == N
    }

    pub fn empty(&mut self) {
        unsafe {
            for i in 0..self.len() {
                self.data[i].assume_init_drop();
            }
        }
        self.size = 0;
    }
}

impl<T, const N: usize> Drop for ArrayVec<T, N> {
    fn drop(&mut self) {
        self.empty();
    }
}

impl<T, const N: usize> Index<usize> for ArrayVec<T, N> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        unsafe {
            if index >= self.size {
                panic!(
                    "index out of bounds: the len is {} but the index is {}",
                    self.size, index
                );
            }
            self.data[index].assume_init_ref()
        }
    }
}

impl<T, const N: usize> IndexMut<usize> for ArrayVec<T, N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe {
            if index >= self.size {
                panic!(
                    "index out of bounds: the len is {} but the index is {}",
                    self.size, index
                );
            }
            self.data[index].assume_init_mut()
        }
    }
}
