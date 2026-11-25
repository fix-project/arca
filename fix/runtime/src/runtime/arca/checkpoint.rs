use super::*;
use hashbrown::HashMap;
use kernel::prelude::{Entry, Page, Table, Word};

pub(super) fn encode(storage: &MemoryStorage, value: Value) -> Handle {
    encode_value(storage, value, &mut HashMap::new())
}

fn encode_value(
    storage: &MemoryStorage,
    value: Value,
    pages: &mut HashMap<(usize, usize), (Page, Handle)>,
) -> Handle {
    let (kind, payload): (&[u8], Handle) = match value {
        Value::Null(_) => (b"Null", storage.add_blob(b"").into()),
        Value::Word(word) => (b"Word", storage.add_blob(&word.read().to_le_bytes()).into()),
        Value::Blob(blob) => (b"Blob", storage.add_blob(&blob).into()),
        Value::Page(page) => {
            let key = (page.inner().as_ptr() as usize, page.len());
            return pages
                .entry(key)
                .or_insert_with(|| {
                    let page = Page::from_inner(page.into_inner().shared());
                    let blob = storage.add_page(page.clone());
                    let encoded =
                        storage.add_tree(&[storage.add_blob(b"Page").into(), blob.into()]);
                    // Keep the backing address live until the checkpoint is encoded.
                    (page, encoded.into())
                })
                .1;
        }
        Value::Tuple(tuple) => {
            let fields: Vec<_> = tuple
                .into_iter()
                .map(|value| encode_value(storage, value, pages))
                .collect();
            (b"Tuple", storage.add_tree(&fields).into())
        }
        Value::Table(mut table) => {
            let length = storage.add_blob(&(table.len() as u64).to_le_bytes());
            let mut entries = Vec::new();
            for index in 0..512 {
                let (mode, value) = match table.take(index).unwrap() {
                    Entry::Null(_) => continue,
                    Entry::ROPage(page) => (0u8, page.into()),
                    Entry::RWPage(page) => (1, page.into()),
                    Entry::ROTable(table) => (0, table.into()),
                    Entry::RWTable(table) => (1, table.into()),
                };
                entries.push(
                    storage
                        .add_tree(&[
                            storage.add_blob(&(index as u64).to_le_bytes()).into(),
                            storage.add_blob(&[mode]).into(),
                            encode_value(storage, value, pages),
                        ])
                        .into(),
                );
            }
            let entries = storage.add_tree(&entries);
            (
                b"Table",
                storage.add_tree(&[length.into(), entries.into()]).into(),
            )
        }
        Value::Function(function) => (b"Function", encode_value(storage, function.read(), pages)),
    };
    storage
        .add_tree(&[storage.add_blob(kind).into(), payload])
        .into()
}

impl<S: Storage + AsRef<MemoryStorage>> Store<S> {
    pub(super) fn decode(&self, value: Handle) -> Value {
        self.decode_value(value, &mut HashMap::new())
    }

    fn decode_value(&self, value: Handle, tables: &mut HashMap<[u8; 32], Table>) -> Value {
        let fields = self.get_tree(Tree::try_from(value).unwrap()).unwrap();
        assert_eq!(fields.len(), 2);
        let kind = Blob::try_from(fields[0]).unwrap();
        let payload = fields[1];
        match kind.literal_bytes().expect("invalid checkpoint kind") {
            b"Null" => Value::default(),
            b"Word" => Word::new(self.integer(payload)).into(),
            b"Blob" => {
                ArcaBlob::new(self.get_blob(Blob::try_from(payload).unwrap()).unwrap()).into()
            }
            b"Page" => {
                let backing = self
                    .get_blob_backing(Blob::try_from(payload).unwrap())
                    .unwrap();
                if let Value::Page(page) = backing.value() {
                    return page.into();
                }
                let mut page = Page::new(backing.len());
                let mut offset = 0;
                backing.for_each_chunk(|bytes| {
                    assert_eq!(page.write(offset, bytes), bytes.len());
                    offset += bytes.len();
                });
                page.into()
            }
            b"Tuple" => {
                let fields = self.get_tree(Tree::try_from(payload).unwrap()).unwrap();
                let mut result = Tuple::new(fields.len());
                for (index, field) in fields.into_iter().enumerate() {
                    result.set(index, self.decode_value(field, tables));
                }
                result.into()
            }
            b"Table" => {
                let key = *value.as_bytes();
                if let Some(table) = tables.get(&key) {
                    return table.clone().into();
                }
                let fields = self.get_tree(Tree::try_from(payload).unwrap()).unwrap();
                let mut table = Table::new(self.integer(fields[0]) as usize);
                for entry in self.get_tree(Tree::try_from(fields[1]).unwrap()).unwrap() {
                    let fields = self.get_tree(Tree::try_from(entry).unwrap()).unwrap();
                    let mode = Blob::try_from(fields[1]).unwrap();
                    let entry = match (mode.literal_bytes(), self.decode_value(fields[2], tables)) {
                        (Some([0]), Value::Page(page)) => Entry::ROPage(page),
                        (Some([1]), Value::Page(page)) => Entry::RWPage(page),
                        (Some([0]), Value::Table(table)) => Entry::ROTable(table),
                        (Some([1]), Value::Table(table)) => Entry::RWTable(table),
                        _ => panic!("invalid checkpoint mapping"),
                    };
                    table.set(self.integer(fields[0]) as usize, entry).unwrap();
                }
                let table = Table::from_inner(table.into_inner().shared());
                tables.insert(key, table.clone());
                table.into()
            }
            b"Function" => Function::new(self.decode_value(payload, tables))
                .expect("invalid checkpoint function")
                .into(),
            _ => panic!("invalid checkpoint value"),
        }
    }
}
