use core::{
    fmt::Write,
    ops::{Deref, DerefMut},
};

pub struct Buffer<const N: usize> {
    index: usize,
    buffer: [u8; N],
}

impl<const N: usize> Buffer<N> {
    pub fn new() -> Self {
        Self {
            index: 0,
            buffer: [0; N],
        }
    }

    fn put8(&mut self, x: u8) {
        if self.index < N {
            self.buffer[self.index] = x;
            self.index += 1;
        }
    }

    fn len(&self) -> usize {
        self.index
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<const N: usize> Default for Buffer<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Write for Buffer<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for x in s.bytes() {
            self.put8(x);
        }
        Ok(())
    }
}

impl<const N: usize> Deref for Buffer<N> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.buffer[..self.index]
    }
}

impl<const N: usize> DerefMut for Buffer<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer[..self.index]
    }
}
