use super::*;
use crate::storage::StorageError;
use hashbrown::HashMap;
use kernel::prelude::Word;

struct Promotion<'a, S> {
    store: &'a Store<S>,
    function: &'a mut Function,
    objects: HashMap<usize, Value>,
    promoted: HashMap<usize, Handle>,
    retained: usize,
    memory: &'a kernel::memory::Budget,
}

impl<S: Storage> Promotion<'_, S> {
    fn promote(&mut self, handle: Handle) -> Handle {
        let Name::Local { address } = handle.name() else {
            return handle;
        };
        let address = address as usize;
        let object = if let Some(object) = self.promoted.get(&address) {
            *object
        } else {
            assert!((0x4000000000..handles::HANDLE_BASE).contains(&address));
            assert!(address.is_multiple_of(1usize << 32));
            let backing = self
                .objects
                .remove(&address)
                .expect("missing local backing");
            let cost = fixhandle::accounting::object(handle.byte_len() as usize);
            let mut data = ImmutableBytes::from_value(backing, handle.byte_len() as usize);
            if handle.is_tree_data() {
                data = self.backing(data);
            }
            if !data.is_empty() && (handle.is_tree_data() || data.len() > 30) {
                self.retained = self.retained.saturating_add(cost);
                if let Some(memory) = self.memory.retain(cost) {
                    data = data.retain(memory);
                }
            }
            let object: Handle = if handle.is_tree_data() {
                unsafe { self.store.add_tree_backing(data) }.into()
            } else {
                self.store.add_blob_backing(data).into()
            };
            self.promoted.insert(address, object);
            object
        };
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
        slots: &[u8],
        memory: &kernel::memory::Budget,
        pins: &mut BlobPins,
    ) -> Result<usize, StorageError> {
        let (slots, remainder) = slots.as_chunks::<4>();
        assert!(remainder.is_empty());
        let mut promotion = Promotion {
            store: self,
            function,
            objects: HashMap::new(),
            promoted: HashMap::new(),
            retained: 0,
            memory,
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
        for slot in slots {
            let address = handles::HANDLE_BASE + u32::from_le_bytes(*slot) as usize * HANDLE_SIZE;
            let handle = handles::resolve(promotion.function, address);
            let promoted = promotion.promote(handle);
            if !handle.same_encoding(&promoted) {
                pins.retain(self, promoted)?;
                handles::insert(promotion.function, address, promoted);
            }
        }
        Ok(promotion.retained)
    }
}
