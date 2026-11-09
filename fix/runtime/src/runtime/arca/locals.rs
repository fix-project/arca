use super::*;
use hashbrown::HashMap;
use kernel::prelude::{Entry, Word};

struct Promotion<'a, S> {
    store: &'a Store<S>,
    function: &'a mut Function,
    objects: HashMap<usize, Value>,
}

impl<S: Storage> Promotion<'_, S> {
    fn promote(&mut self, handle: Handle) -> Handle {
        let Name::Local { address } = handle.name() else {
            return handle;
        };
        let address = address as usize;
        let mut object = handles::resolve(self.function, address);
        if matches!(object.name(), Name::Local { .. }) {
            assert!(
                matches!(object.name(), Name::Local { address: source } if source as usize == address)
            );
            let backing = self
                .objects
                .remove(&address)
                .expect("missing local backing");
            object = match backing {
                Value::Blob(blob) => {
                    assert!(Blob::try_from(object).is_ok());
                    assert_eq!(object.byte_len() as usize, blob.len());
                    self.store.add_blob(&blob).into()
                }
                backing => {
                    assert!(Tree::try_from(object).is_ok());
                    let data = ImmutableBytes::from_value(backing, object.byte_len() as usize);
                    let data = self.backing(data);
                    unsafe { self.store.add_tree_backing(data) }.into()
                }
            };
            handles::insert(self.function, address, object);
        }
        let mut bytes = *object.as_bytes();
        bytes[31] |= handle.as_bytes()[31] & 31;
        unsafe { Handle::parse(bytes) }.expect("invalid promoted handle")
    }

    fn cells(&mut self, bytes: &mut [u8]) -> bool {
        let mut changed = false;
        for cell in bytes.as_chunks_mut::<HANDLE_SIZE>().0 {
            let handle = unsafe { Handle::parse(*cell) }.expect("invalid local backing");
            let promoted = self.promote(handle);
            if !handle.same_encoding(&promoted) {
                *cell = *promoted.as_bytes();
                changed = true;
            }
        }
        changed
    }

    fn backing(&mut self, mut data: ImmutableBytes) -> ImmutableBytes {
        for offset in (0..data.len()).step_by(4096) {
            let mut bytes = [0; 4096];
            let used = (data.len() - offset).min(bytes.len());
            {
                let (page, position) = data.page(offset);
                assert_eq!(page.read(position, &mut bytes[..used]), used);
            }
            if self.cells(&mut bytes[..used]) {
                data = data.with_bytes(offset, &bytes[..used]);
            }
        }
        data
    }
}

impl<S: Storage> Store<S> {
    pub(super) fn lift(
        &self,
        function: &mut Function,
        objects: Tuple,
        aliases: Tuple,
        tables: Tuple,
    ) {
        let mut promotion = Promotion {
            store: self,
            function,
            objects: HashMap::new(),
        };
        for record in objects {
            let mut record: Tuple = record.try_into().expect("invalid local object");
            let cell: Word = record.take(0).try_into().unwrap();
            assert!(
                promotion
                    .objects
                    .insert(cell.read() as usize, record.take(1))
                    .is_none()
            );
        }
        for cell in aliases {
            let cell: Word = cell.try_into().unwrap();
            let cell = cell.read() as usize;
            let handle = handles::resolve(promotion.function, cell);
            let promoted = promotion.promote(handle);
            if !handle.same_encoding(&promoted) {
                handles::insert(promotion.function, cell, promoted);
            }
        }
        for table in tables {
            let mut table: Tuple = table.try_into().unwrap();
            let address: Word = table.take(0).try_into().unwrap();
            let length: Word = table.take(1).try_into().unwrap();
            let address = address.read() as usize;
            let length = length.read() as usize * HANDLE_SIZE;
            for offset in (0..length).step_by(4096) {
                let mut bytes = [0; 4096];
                let mut changed = [false; 4096 / HANDLE_SIZE];
                let used = (length - offset).min(bytes.len());
                {
                    assert_eq!(
                        handles::read(promotion.function, address + offset, &mut bytes[..used]),
                        used
                    );
                }
                for (index, cell) in bytes[..used]
                    .as_chunks_mut::<HANDLE_SIZE>()
                    .0
                    .iter_mut()
                    .enumerate()
                {
                    let handle = unsafe { Handle::parse(*cell) }.expect("invalid local backing");
                    let promoted = promotion.promote(handle);
                    changed[index] = !handle.same_encoding(&promoted);
                    *cell = *promoted.as_bytes();
                }
                if changed.iter().any(|changed| *changed) {
                    let mappings = promotion
                        .function
                        .inner_mut()
                        .arca_mut()
                        .unwrap()
                        .mappings_mut();
                    let result = mappings.update(address + offset, 4096, |entry| {
                        let (Entry::ROPage(mut page) | Entry::RWPage(mut page)) = entry else {
                            panic!("invalid handle mapping");
                        };
                        for (index, cell) in bytes[..used]
                            .as_chunks::<HANDLE_SIZE>()
                            .0
                            .iter()
                            .enumerate()
                        {
                            if changed[index] {
                                assert_eq!(page.write(index * HANDLE_SIZE, cell), HANDLE_SIZE);
                            }
                        }
                        (Entry::RWPage(page), ())
                    });
                    assert!(result.is_ok());
                }
            }
        }
        assert!(promotion.objects.is_empty(), "unpromoted local objects");
    }
}
