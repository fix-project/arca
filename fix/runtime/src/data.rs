use arca::Runtime as _;
use core::cmp;
use core::panic;
use core::simd::u8x32;
use fixhandle::rawhandle::BitPack;
use fixhandle::rawhandle::FixHandle;
use kernel::prelude::*;
use kernel::types::Runtime;
use kernel::types::Table;

const MAXSIZE: usize = 1 << 32;

#[derive(Debug, Clone)]
pub struct RawData {
    data: Table,
    length: usize,
}

impl RawData {
    fn new(length: usize) -> Self {
        if length > MAXSIZE {
            panic!("Data larger than maximum size")
        }
        Self {
            data: Table::new(MAXSIZE),
            length,
        }
    }

    fn create(data: &[u8]) -> Self {
        let mut inner = RawData::new(data.len());
        let pagesize = inner.data.len() / 512;
        for i in 0..(data.len() + pagesize - 1) / pagesize {
            let mut page = Runtime::create_page(pagesize);
            Runtime::write_page(&mut page, 0, &data[i * pagesize..]);
            Runtime::set_table(&mut inner.data, i, arca::Entry::ROPage(page))
                .expect("Unable to set entry");
        }
        inner
    }

    fn get(&self, start: usize, buf: &mut [u8]) {
        let pagesize = self.data.len() / 512;
        let mut curr_start = start;
        let mut index = 0;
        while curr_start - start < buf.len() {
            // Advance to the end of current page
            let mut curr_end = (curr_start / pagesize + 1) * pagesize;
            curr_end = cmp::min(curr_end, start + buf.len());
            match Runtime::get_table(&self.data, index)
                .expect("Page to get is out of the MAXSIZE range")
            {
                arca::Entry::Null(_) => (),
                arca::Entry::ROPage(page) | arca::Entry::RWPage(page) => {
                    Runtime::read_page(
                        &page,
                        curr_start % pagesize,
                        &mut buf[curr_start - start..curr_end - start],
                    );
                }
                arca::Entry::ROTable(_) => todo!(),
                arca::Entry::RWTable(_) => todo!(),
            }

            index += 1;
            curr_start = curr_end;
        }
    }
}

impl From<RawData> for Table {
    fn from(val: RawData) -> Self {
        val.data
    }
}

#[derive(Debug, Clone)]
pub struct BlobData {
    inner: RawData,
}

impl BlobData {
    pub fn new(data: Table, length: usize) -> Self {
        let inner = RawData { data, length };
        Self { inner }
    }

    pub fn create(data: &[u8]) -> Self {
        let inner = RawData::create(data);
        Self { inner }
    }

    pub fn len(&self) -> usize {
        self.inner.length
    }

    pub fn get(&self, buf: &mut [u8]) {
        self.inner.get(0, buf)
    }
}

impl From<BlobData> for RawData {
    fn from(val: BlobData) -> Self {
        val.inner
    }
}

impl From<RawData> for BlobData {
    fn from(value: RawData) -> Self {
        Self { inner: value }
    }
}

#[derive(Debug, Clone)]
pub struct TreeData {
    inner: RawData,
}

impl TreeData {
    pub fn new(data: Table, length: usize) -> Self {
        let inner = RawData { data, length };
        Self { inner }
    }

    pub fn create(data: &[FixHandle]) -> Self {
        let mut buffer = vec![0u8; data.len() * 32];
        for (idx, i) in data.iter().enumerate() {
            let raw = i.pack();
            buffer.as_mut_slice()[idx * 32..(idx + 1) * 32].copy_from_slice(raw.as_array());
        }

        let inner = RawData::create(&buffer);
        Self { inner }
    }

    pub fn len(&self) -> usize {
        self.inner.length / 32
    }

    pub fn get(&self, idx: usize) -> FixHandle {
        let mut buffer = [0u8; 32];
        self.inner.get(idx * 32, &mut buffer);
        FixHandle::unpack(u8x32::from_array(buffer))
    }
}

impl From<TreeData> for RawData {
    fn from(val: TreeData) -> Self {
        val.inner
    }
}

impl From<RawData> for TreeData {
    fn from(value: RawData) -> Self {
        Self { inner: value }
    }
}
