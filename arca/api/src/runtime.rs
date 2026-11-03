use super::prelude::*;

pub trait Runtime: Sized + core::fmt::Debug + Eq + PartialEq + Clone {
    type Null: Clone + core::fmt::Debug + Eq + PartialEq;
    type Word: Clone + core::fmt::Debug + Eq + PartialEq;
    type Blob: Clone + core::fmt::Debug + Eq + PartialEq;
    type Tuple: Clone + core::fmt::Debug + Eq + PartialEq;
    type Page: Clone + core::fmt::Debug + Eq + PartialEq;
    type Table: Clone + core::fmt::Debug + Eq + PartialEq;
    type Function: Clone + core::fmt::Debug + Eq + PartialEq;

    type Error: core::fmt::Debug;

    fn create_null() -> Null<Self>;
    fn create_word(word: u64) -> Word<Self>;
    fn create_blob(bytes: &[u8]) -> Blob<Self>;
    fn create_tuple(len: usize) -> Tuple<Self>;
    fn create_page(len: usize) -> Page<Self>;
    fn create_table(len: usize) -> Table<Self>;
    /// Creates a writable zero-filled mapping of 4 KiB, 2 MiB, or 1 GiB,
    /// backed by 4 KiB data pages.
    fn create_zeroed_entry(len: usize) -> Entry<Self> {
        assert!([1 << 12, 1 << 21, 1 << 30].contains(&len));
        if len == 4096 {
            return Entry::RWPage(Self::create_page(len));
        }
        let mut table = Self::create_table(len);
        for index in 0..512 {
            table
                .set(index, Self::create_zeroed_entry(len / 512))
                .expect("zero-filled table entry");
        }
        Entry::RWTable(table)
    }
    fn create_function(data: Value<Self>) -> Result<Function<Self>, Self::Error>;

    fn value_len(value: ValueRef<Self>) -> usize;

    fn read_word(word: &Word<Self>) -> u64;
    fn read_blob(blob: &Blob<Self>, offset: usize, buf: &mut [u8]) -> usize;
    fn read_page(page: &Page<Self>, offset: usize, buf: &mut [u8]) -> usize;
    fn read_function(function: Function<Self>) -> Value<Self>;

    fn write_blob(blob: &mut Blob<Self>, offset: usize, buf: &[u8]) -> usize;
    fn write_page(page: &mut Page<Self>, offset: usize, buf: &[u8]) -> usize;

    fn get_tuple(tuple: &Tuple<Self>, index: usize) -> Result<Value<Self>, Self::Error>;
    fn set_tuple(
        tuple: &mut Tuple<Self>,
        index: usize,
        value: Value<Self>,
    ) -> Result<Value<Self>, Self::Error>;

    fn get_table(table: &Table<Self>, index: usize) -> Result<Entry<Self>, Self::Error>;
    fn set_table(
        table: &mut Table<Self>,
        index: usize,
        entry: Entry<Self>,
    ) -> Result<Entry<Self>, Self::Error>;

    fn apply_function(function: Function<Self>, argument: Value<Self>) -> Function<Self>;
    fn force_function(function: Function<Self>) -> Value<Self>;
    fn is_function_arcane(function: &Function<Self>) -> bool;
    fn call_with_current_continuation(function: Function<Self>) -> Value<Self>;

    #[cfg(feature = "alloc")]
    fn with_blob_as_ref<T>(blob: &Blob<Self>, f: impl FnOnce(&[u8]) -> T) -> T {
        let mut buf = vec![0; blob.len()];
        Self::read_blob(blob, 0, &mut buf);
        f(&buf)
    }

    #[cfg(feature = "alloc")]
    fn with_tuple_as_ref<T>(tuple: &Tuple<Self>, f: impl FnOnce(&[Value<Self>]) -> T) -> T {
        let v: Vec<Value<Self>> = tuple.iter().collect();
        f(&v)
    }

    #[cfg(feature = "alloc")]
    fn with_page_as_ref<T>(page: &Page<Self>, f: impl FnOnce(&[u8]) -> T) -> T {
        let mut buf = vec![0; page.len()];
        Self::read_page(page, 0, &mut buf);
        f(&buf)
    }

    #[cfg(feature = "alloc")]
    fn with_page_as_mut<T>(page: &mut Page<Self>, f: impl FnOnce(&mut [u8]) -> T) -> T {
        let mut buf = vec![0; page.len()];
        Self::read_page(page, 0, &mut buf);
        let result = f(&mut buf);
        Self::write_page(page, 0, &buf);
        result
    }

    #[cfg(feature = "alloc")]
    fn with_table_as_ref<T>(table: &Table<Self>, f: impl FnOnce(&[Entry<Self>]) -> T) -> T {
        let v: Vec<Entry<Self>> = table.iter().collect();
        f(&v)
    }
}
