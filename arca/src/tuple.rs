use super::prelude::*;

impl<R: Runtime> Tuple<R> {
    pub fn new(len: usize) -> Self {
        R::create_tuple(len)
    }

    pub fn get(&self, idx: usize) -> Value<R> {
        R::get_tuple(self, idx).unwrap()
    }

    pub fn set(&mut self, idx: usize, value: impl Into<Value<R>>) -> Value<R> {
        R::set_tuple(self, idx, value.into()).unwrap()
    }

    pub fn take(&mut self, idx: usize) -> Value<R> {
        let replacement = Value::default();
        self.set(idx, replacement)
    }

    pub fn swap(&mut self, idx: usize, value: &mut Value<R>) {
        let mut replacement = self.take(idx);
        core::mem::swap(&mut replacement, value);
        self.set(idx, replacement);
    }

    pub fn iter(&self) -> TupleIter<'_, R> {
        TupleIter {
            tuple: self,
            len: self.len(),
            index: 0,
        }
    }

    #[cfg(feature = "alloc")]
    pub fn with_ref<T>(&self, f: impl FnOnce(&[Value<R>]) -> T) -> T {
        R::with_tuple_as_ref(self, f)
    }
}

impl<R: Runtime, A: Into<Value<R>>, B: Into<Value<R>>> From<(A, B)> for Tuple<R> {
    fn from(value: (A, B)) -> Self {
        let mut tuple = Tuple::new(2);
        tuple.set(0, value.0);
        tuple.set(1, value.1);
        tuple
    }
}

impl<R: Runtime, A: Into<Value<R>>, B: Into<Value<R>>, C: Into<Value<R>>> From<(A, B, C)>
    for Tuple<R>
{
    fn from(value: (A, B, C)) -> Self {
        let mut tuple = Tuple::new(3);
        tuple.set(0, value.0);
        tuple.set(1, value.1);
        tuple.set(2, value.2);
        tuple
    }
}

impl<R: Runtime, A: Into<Value<R>>, B: Into<Value<R>>, C: Into<Value<R>>, D: Into<Value<R>>>
    From<(A, B, C, D)> for Tuple<R>
{
    fn from(value: (A, B, C, D)) -> Self {
        let mut tuple = Tuple::new(4);
        tuple.set(0, value.0);
        tuple.set(1, value.1);
        tuple.set(2, value.2);
        tuple.set(3, value.3);
        tuple
    }
}

impl<R: Runtime> From<&mut [Value<R>]> for Tuple<R> {
    fn from(value: &mut [Value<R>]) -> Self {
        let mut tuple = Tuple::new(value.len());
        for (i, x) in value.iter_mut().enumerate() {
            let mut value = Value::default();
            core::mem::swap(&mut value, x);
            tuple.set(i, value);
        }
        tuple
    }
}

pub struct TupleIter<'a, R: Runtime> {
    tuple: &'a Tuple<R>,
    len: usize,
    index: usize,
}

impl<'a, R: Runtime> Iterator for TupleIter<'a, R> {
    type Item = Value<R>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        let next = self.tuple.get(self.index);
        self.index += 1;
        Some(next)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len - self.index, Some(self.len - self.index))
    }
}

impl<R: Runtime> ExactSizeIterator for TupleIter<'_, R> {}

#[derive(Debug, Clone)]
pub struct TupleIntoIter<R: Runtime> {
    tuple: Tuple<R>,
    len: usize,
    index: usize,
}

impl<R: Runtime> IntoIterator for Tuple<R> {
    type Item = Value<R>;

    type IntoIter = TupleIntoIter<R>;

    fn into_iter(self) -> Self::IntoIter {
        let len = self.len();
        TupleIntoIter {
            tuple: self,
            len,
            index: 0,
        }
    }
}

impl<R: Runtime> Iterator for TupleIntoIter<R> {
    type Item = Value<R>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        let next = self.tuple.take(self.index);
        self.index += 1;
        Some(next)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len - self.index, Some(self.len - self.index))
    }
}

impl<R: Runtime> ExactSizeIterator for TupleIntoIter<R> {}

impl<R: Runtime, V: Into<Value<R>>> FromIterator<V> for Tuple<R> {
    fn from_iter<T: IntoIterator<Item = V>>(iter: T) -> Self {
        let iter = iter.into_iter();
        let hint = iter.size_hint();
        let mut n = hint.0;
        let mut tuple: Tuple<R> = Tuple::new(n);
        let mut max_i = 0;
        for (i, x) in iter.enumerate() {
            if i < n {
                tuple.set(i, x);
                max_i = i;
            } else {
                let new_n = if let Some(upper) = hint.1 {
                    upper
                } else {
                    n * 2
                };
                let mut new_tuple = Tuple::new(new_n);
                for (i, x) in tuple.into_iter().enumerate() {
                    new_tuple.set(i, x);
                }
                n = new_n;
                tuple = new_tuple;
            }
        }
        if n > 0 && max_i < (n - 1) {
            let mut final_tuple = Tuple::new(max_i + 1);
            for (i, x) in tuple.into_iter().enumerate().take(max_i) {
                final_tuple.set(i, x);
            }
            tuple = final_tuple;
        }
        tuple
    }
}
