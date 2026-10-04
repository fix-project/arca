use kernel::prelude::{Box, Entry, Page, Table, Tuple, Value, Vec, vec};

const CHUNK_SIZE: usize = 1 << 30;

#[derive(Clone, Debug)]
pub struct ImmutableBytes {
    len: usize,
    tables: Vec<Table>,
}

impl ImmutableBytes {
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
            tables,
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

    pub fn tables(&self) -> &[Table] {
        &self.tables
    }

    pub fn value(&self) -> Value {
        backing_value(&self.tables)
    }

    pub fn to_bytes(&self) -> Box<[u8]> {
        let mut bytes = vec![0; self.len];
        for (index, chunk) in bytes.chunks_mut(4096).enumerate() {
            let position = index * 4096;
            let mut table = self.tables[position / CHUNK_SIZE].clone();
            let mut offset = position % CHUNK_SIZE;
            loop {
                let span = table.len() / 512;
                match table.get(offset / span).expect("byte mapping") {
                    Entry::ROTable(child) | Entry::RWTable(child) => {
                        table = child;
                        offset %= span;
                    }
                    Entry::ROPage(page) => {
                        assert_eq!(page.read(offset % span, chunk), chunk.len());
                        break;
                    }
                    _ => panic!("invalid immutable byte mapping"),
                }
            }
        }
        bytes.into_boxed_slice()
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
