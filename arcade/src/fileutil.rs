use core::cmp::min;

use kernel::prelude::*;

pub fn buffer_to_table(buf: &[u8]) -> Table {
    let mut table = Table::new(buf.len());
    let pagesize = match table.clone().into_inner() {
        kernel::types::internal::Table::Table1GB(_) => 2 * 1024 * 1024,
        kernel::types::internal::Table::Table2MB(_) => panic!(),
        kernel::types::internal::Table::Table512GB(_) => panic!(),
    } as usize;

    for i in 0..buf.len().div_ceil(pagesize) {
        let mut page = Page::new(pagesize);
        page.write(0, &buf[i * pagesize..min((i + 1) * pagesize, buf.len())]);
        let _ = table.set(i, Entry::ROPage(page));
    }

    table
}

pub fn read_table_to_buffer(table: &Table, offset: usize, mut buf: &mut [u8]) -> usize {
    let mut acc = 0;
    let mut read = 0;
    for p in table.iter() {
        if buf.is_empty() {
            return read;
        }

        match &p {
            arca::Entry::ROPage(page) | arca::Entry::RWPage(page) => {
                if acc + page.len() < offset {
                    acc += page.len();
                    continue;
                }

                if acc < offset {
                    // Partially read current page
                    let n = page.read(offset - acc, buf);
                    read += n;
                    buf = &mut buf[n..];
                    acc = offset;
                    continue;
                }

                let n = page.read(0, buf);
                read += n;
                buf = &mut buf[n..];
            }
            arca::Entry::ROTable(_) | arca::Entry::RWTable(_) => panic!(),
            arca::Entry::Null(_) => return read,
        }
    }

    read
}
