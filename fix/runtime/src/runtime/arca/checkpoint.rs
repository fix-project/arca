use super::*;
use crate::storage::ImmutableBytes;
use kernel::prelude::{Entry, Page, Table, Word};

pub(super) fn encode(storage: &dyn Storage, value: Value) -> Handle {
    let (kind, payload): (&[u8], Handle) = match value {
        Value::Null(_) => (b"Null", storage.add_blob(b"").into()),
        Value::Word(word) => (b"Word", storage.add_blob(&word.read().to_le_bytes()).into()),
        Value::Blob(blob) => (b"Blob", storage.add_blob(&blob).into()),
        Value::Page(page) => (
            b"Page",
            storage
                .add_blob_backing(ImmutableBytes::from_page(page))
                .into(),
        ),
        Value::Tuple(tuple) => {
            let fields: Vec<_> = tuple
                .into_iter()
                .map(|value| encode(storage, value))
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
                            encode(storage, value),
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
        Value::Function(function) => (b"Function", encode(storage, function.read())),
    };
    storage
        .add_tree(&[storage.add_blob(kind).into(), payload])
        .into()
}

impl<S: Storage + AsRef<MemoryStorage>> Store<S> {
    pub(super) fn decode(&self, value: Handle) -> Value {
        let fields = self.get_tree(Tree::try_from(value).unwrap()).unwrap();
        assert_eq!(fields.len(), 2);
        let kind = self.get_blob(Blob::try_from(fields[0]).unwrap()).unwrap();
        let payload = fields[1];
        match kind.as_ref() {
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
                    result.set(index, self.decode(field));
                }
                result.into()
            }
            b"Table" => {
                let fields = self.get_tree(Tree::try_from(payload).unwrap()).unwrap();
                let mut table = Table::new(self.integer(fields[0]) as usize);
                for entry in self.get_tree(Tree::try_from(fields[1]).unwrap()).unwrap() {
                    let fields = self.get_tree(Tree::try_from(entry).unwrap()).unwrap();
                    let mode = self.get_blob(Blob::try_from(fields[1]).unwrap()).unwrap();
                    let entry = match (mode.as_ref(), self.decode(fields[2])) {
                        ([0], Value::Page(page)) => Entry::ROPage(page),
                        ([1], Value::Page(page)) => Entry::RWPage(page),
                        ([0], Value::Table(table)) => Entry::ROTable(table),
                        ([1], Value::Table(table)) => Entry::RWTable(table),
                        _ => panic!("invalid checkpoint mapping"),
                    };
                    table.set(self.integer(fields[0]) as usize, entry).unwrap();
                }
                table.into()
            }
            b"Function" => Function::new(self.decode(payload))
                .expect("invalid checkpoint function")
                .into(),
            _ => panic!("invalid checkpoint value"),
        }
    }
}
