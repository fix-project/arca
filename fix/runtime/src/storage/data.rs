use kernel::prelude::{Box, Entry, Page, Table, Tuple, Value, Vec, vec};

const CHUNK_SIZE: usize = 1 << 30;

#[derive(Clone, Debug)]
pub struct ImmutableBytes {
    len: usize,
    backing: Backing,
}

#[derive(Clone, Debug)]
enum Backing {
    Page(Page),
    Tables(Vec<Table>),
}

impl ImmutableBytes {
    pub fn from_tables(tables: Vec<Table>, len: usize) -> Self {
        assert!(len <= 4 * CHUNK_SIZE);
        assert_eq!(tables.len(), len.div_ceil(CHUNK_SIZE).max(1));
        let tables = tables
            .into_iter()
            .enumerate()
            .map(|(index, table)| {
                let used = (len - index * CHUNK_SIZE).min(CHUNK_SIZE);
                assert!(table.len() >= used && table.len() <= CHUNK_SIZE);
                if len > CHUNK_SIZE {
                    assert_eq!(table.len(), CHUNK_SIZE);
                }
                Self::freeze(table, used)
            })
            .collect();
        Self {
            len,
            backing: Backing::Tables(tables),
        }
    }

    fn freeze(mut table: Table, len: usize) -> Table {
        let span = table.len() / 512;
        for index in 0..512 {
            let entry = table.take(index).expect("byte mapping");
            let offset = index * span;
            if offset >= len {
                continue;
            }
            let used = (len - offset).min(span);
            let entry = match entry {
                Entry::ROTable(child) | Entry::RWTable(child) => {
                    Entry::ROTable(Self::freeze(child, used))
                }
                Entry::ROPage(mut page) | Entry::RWPage(mut page) => {
                    assert_eq!(page.len(), 4096);
                    if used < page.len()
                        && page.with_ref(|bytes| bytes[used..].iter().any(|byte| *byte != 0))
                    {
                        page.with_mut(|bytes| bytes[used..].fill(0));
                    }
                    Entry::ROPage(page)
                }
                Entry::Null(_) => panic!("missing byte mapping"),
            };
            table.set(index, entry).expect("byte mapping");
        }
        Table::from_inner(table.into_inner().shared())
    }

    pub fn new(bytes: &[u8]) -> Self {
        let mut tables = Vec::new();
        if bytes.is_empty() {
            tables.push(Self::share_mapping(Table::new(0)));
        }
        for chunk in bytes.chunks(CHUNK_SIZE) {
            let span = if bytes.len() > CHUNK_SIZE {
                CHUNK_SIZE
            } else {
                chunk.len()
            };
            let mut mapping = Table::new(span);
            for (index, bytes) in chunk.chunks(4096).enumerate() {
                let mut page = Page::new(4096);
                assert_eq!(page.write(0, bytes), bytes.len());
                assert!(mapping.map(index * 4096, Entry::ROPage(page)).is_ok());
            }
            tables.push(Self::share_mapping(mapping));
        }
        Self {
            len: bytes.len(),
            backing: Backing::Tables(tables),
        }
    }

    fn share_mapping(mut table: Table) -> Table {
        if table.len() > 1 << 21 {
            for index in 0..512 {
                let entry = table.take(index).expect("byte mapping");
                let entry = match entry {
                    Entry::RWTable(child) => Entry::ROTable(Self::share_mapping(child)),
                    entry => entry,
                };
                table.set(index, entry).expect("byte mapping");
            }
        }
        Table::from_inner(table.into_inner().shared())
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn with_bytes(self, offset: usize, bytes: &[u8]) -> Self {
        let end = offset
            .checked_add(bytes.len())
            .expect("byte range overflow");
        assert!(offset <= self.len && end <= 4 * CHUNK_SIZE);
        if bytes.is_empty() {
            return self;
        }
        let len = self.len.max(end);
        let mut tables = match self.backing {
            Backing::Tables(tables) => tables,
            Backing::Page(page) => match Self::from_page(page).mapped_value() {
                Value::Table(table) => vec![table],
                Value::Tuple(tables) => tables
                    .into_iter()
                    .map(|table| table.try_into().unwrap())
                    .collect(),
                _ => unreachable!(),
            },
        };
        while tables.len() < len.div_ceil(CHUNK_SIZE) {
            tables.push(Table::new(CHUNK_SIZE));
        }
        let mut position = offset;
        let mut remaining = bytes;
        while !remaining.is_empty() {
            let table = &mut tables[position / CHUNK_SIZE];
            let base = (position % CHUNK_SIZE) & !4095;
            let mut page = if base < table.len() {
                match table.unmap(base).expect("byte mapping") {
                    Entry::ROPage(page) | Entry::RWPage(page) => page,
                    Entry::Null(_) => Page::new(4096),
                    _ => unreachable!(),
                }
            } else {
                Page::new(4096)
            };
            let size = remaining.len().min(4096 - position % 4096);
            assert_eq!(page.write(position % 4096, &remaining[..size]), size);
            assert!(table.map(base, Entry::ROPage(page)).is_ok());
            position += size;
            remaining = &remaining[size..];
        }
        Self {
            len,
            backing: Backing::Tables(tables.into_iter().map(Self::share_mapping).collect()),
        }
    }

    pub fn from_page(page: Page) -> Self {
        Self {
            len: page.len(),
            backing: Backing::Page(Page::from_inner(page.into_inner().shared())),
        }
    }

    pub fn value(&self) -> Value {
        match &self.backing {
            Backing::Page(page) => page.clone().into(),
            Backing::Tables(tables) => backing_value(tables),
        }
    }

    pub fn mapped_value(&self) -> Value {
        let mut tables = Vec::new();
        for base in (0..self.len.max(1)).step_by(CHUNK_SIZE) {
            let len = (self.len - base).min(CHUNK_SIZE);
            let mut table = Table::new(if self.len > CHUNK_SIZE {
                CHUNK_SIZE
            } else {
                len
            });
            for offset in (0..len).step_by(4096) {
                let (page, position) = self.page(base + offset);
                let page = if page.len() == 4096 {
                    page
                } else {
                    let mut bytes = [0; 4096];
                    let used = (len - offset).min(bytes.len());
                    assert_eq!(page.read(position, &mut bytes[..used]), used);
                    let mut copy = Page::new(4096);
                    assert_eq!(copy.write(0, &bytes), bytes.len());
                    copy
                };
                assert!(table.map(offset, Entry::ROPage(page)).is_ok());
            }
            tables.push(Self::share_mapping(table));
        }
        backing_value(&tables)
    }

    pub fn page(&self, offset: usize) -> (Page, usize) {
        assert!(offset < self.len);
        let mut table = match &self.backing {
            Backing::Page(page) => return (page.clone(), offset),
            Backing::Tables(tables) => tables[offset / CHUNK_SIZE].clone(),
        };
        let mut offset = offset % CHUNK_SIZE;
        loop {
            let span = table.len() / 512;
            match table.get(offset / span).expect("byte mapping") {
                Entry::ROTable(child) | Entry::RWTable(child) => {
                    table = child;
                    offset %= span;
                }
                Entry::ROPage(page) => return (page, offset % span),
                _ => panic!("invalid immutable byte mapping"),
            }
        }
    }

    pub fn to_bytes(&self) -> Box<[u8]> {
        let mut bytes = vec![0; self.len];
        let mut position = 0;
        self.for_each_chunk(|chunk| {
            bytes[position..position + chunk.len()].copy_from_slice(chunk);
            position += chunk.len();
        });
        bytes.into_boxed_slice()
    }

    pub fn for_each_chunk(&self, mut f: impl FnMut(&[u8])) {
        match &self.backing {
            Backing::Page(page) => {
                page.with_ref(f);
                return;
            }
            Backing::Tables(_) => {}
        }
        for position in (0..self.len).step_by(4096) {
            let (page, offset) = self.page(position);
            let size = (self.len - position).min(4096);
            page.with_ref(|bytes| f(&bytes[offset..offset + size]));
        }
    }
}

pub fn backing_value(tables: &[Table]) -> Value {
    assert!(!tables.is_empty() && tables.len() <= 4);
    if tables.len() == 1 {
        return tables[0].clone().into();
    }
    let mut chunks = Tuple::new(tables.len());
    for (index, table) in tables.iter().enumerate() {
        assert_eq!(table.len(), CHUNK_SIZE);
        chunks.set(index, table.clone());
    }
    chunks.into()
}
