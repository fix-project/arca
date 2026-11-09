use super::*;
use crate::storage::{CanonicalHandle, ImmutableBytes, StorageError};
use kernel::kthread::KMutex;
use kernel::prelude::{Arc, Box};

type Resident = Arc<KMutex<Option<Function>>>;

#[derive(Debug)]
pub(super) struct Store<S> {
    inner: S,
    pub(super) profile: core::sync::atomic::AtomicBool,
    pub(super) trust: trust::Trust,
    pub(super) image_cache_id: usize,
    pub(super) images: KMutex<hashbrown::HashMap<[u8; 32], Resident>>,
    pub(super) applications: KMutex<hashbrown::HashMap<ProfileKey, ProfileTiming>>,
    continuations: KMutex<hashbrown::HashMap<[u8; 32], (Resident, usize, Tree)>>,
    captures: AtomicUsize,
    elf_loads: AtomicUsize,
    elf_load_ns: core::sync::atomic::AtomicU64,
    kernel_calls: AtomicUsize,
    tree_gets: AtomicUsize,
    handle_reads: AtomicUsize,
    privilege_checks: AtomicUsize,
    force_ns: core::sync::atomic::AtomicU64,
    tree_get_ns: core::sync::atomic::AtomicU64,
    handle_read_ns: core::sync::atomic::AtomicU64,
    privilege_ns: core::sync::atomic::AtomicU64,
    attach_tree_ns: core::sync::atomic::AtomicU64,
    lifts: AtomicUsize,
    blob_reads: AtomicUsize,
    effects: AtomicUsize,
    returns: AtomicUsize,
    blob_gets: AtomicUsize,
}

impl<S: Storage + AsRef<MemoryStorage>> Store<S> {
    pub(super) fn memory(&self) -> &MemoryStorage {
        self.inner.as_ref()
    }

    pub(super) fn new(inner: S) -> Self {
        Self {
            inner,
            profile: core::sync::atomic::AtomicBool::new(false),
            applications: KMutex::new(hashbrown::HashMap::new()),
            images: KMutex::new(hashbrown::HashMap::new()),
            trust: trust::Trust::default(),
            image_cache_id: {
                static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
                NEXT_ID
                    .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                    .expect("runtime identity exhausted")
            },
            continuations: KMutex::new(hashbrown::HashMap::new()),
            captures: AtomicUsize::new(0),
            elf_loads: AtomicUsize::new(0),
            elf_load_ns: core::sync::atomic::AtomicU64::new(0),
            kernel_calls: AtomicUsize::new(0),
            tree_gets: AtomicUsize::new(0),
            handle_reads: AtomicUsize::new(0),
            privilege_checks: AtomicUsize::new(0),
            force_ns: core::sync::atomic::AtomicU64::new(0),
            tree_get_ns: core::sync::atomic::AtomicU64::new(0),
            handle_read_ns: core::sync::atomic::AtomicU64::new(0),
            privilege_ns: core::sync::atomic::AtomicU64::new(0),
            attach_tree_ns: core::sync::atomic::AtomicU64::new(0),
            lifts: AtomicUsize::new(0),
            blob_reads: AtomicUsize::new(0),
            effects: AtomicUsize::new(0),
            returns: AtomicUsize::new(0),
            blob_gets: AtomicUsize::new(0),
        }
    }

    pub(super) fn capture_count(&self) -> usize {
        self.captures.load(Ordering::Relaxed)
    }

    pub(super) fn elf_loads(&self) -> (usize, u64) {
        (
            self.elf_loads.load(Ordering::Relaxed),
            self.elf_load_ns.load(Ordering::Relaxed),
        )
    }

    pub(super) fn record_elf_load(&self, started: Option<core::time::Duration>) {
        if let Some(started) = started {
            self.elf_loads.fetch_add(1, Ordering::Relaxed);
            self.elf_load_ns.fetch_add(
                kernel::tsc::read().saturating_sub(started).as_nanos() as u64,
                Ordering::Relaxed,
            );
        }
    }

    pub(super) fn kernel_calls(&self) -> (usize, usize, usize, usize) {
        (
            self.kernel_calls.load(Ordering::Relaxed),
            self.tree_gets.load(Ordering::Relaxed),
            self.handle_reads.load(Ordering::Relaxed),
            self.privilege_checks.load(Ordering::Relaxed),
        )
    }
    pub(super) fn record_kernel_call(&self, name: &[u8]) {
        self.kernel_calls.fetch_add(1, Ordering::Relaxed);
        match name {
            b"get_tree_element" => {
                self.tree_gets.fetch_add(1, Ordering::Relaxed);
            }
            b"unsound_handle_bytes" => {
                self.handle_reads.fetch_add(1, Ordering::Relaxed);
            }
            b"is_privileged" => {
                self.privilege_checks.fetch_add(1, Ordering::Relaxed);
            }
            b"lift" => {
                self.lifts.fetch_add(1, Ordering::Relaxed);
            }
            b"read_blob" => {
                self.blob_reads.fetch_add(1, Ordering::Relaxed);
            }
            b"effect" => {
                self.effects.fetch_add(1, Ordering::Relaxed);
            }
            b"return" => {
                self.returns.fetch_add(1, Ordering::Relaxed);
            }
            b"get_blob" => {
                self.blob_gets.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    pub(super) fn other_calls(&self) -> [usize; 5] {
        [
            self.lifts.load(Ordering::Relaxed),
            self.blob_reads.load(Ordering::Relaxed),
            self.effects.load(Ordering::Relaxed),
            self.returns.load(Ordering::Relaxed),
            self.blob_gets.load(Ordering::Relaxed),
        ]
    }

    pub(super) fn call_times(&self) -> [u64; 5] {
        [
            self.force_ns.load(Ordering::Relaxed),
            self.tree_get_ns.load(Ordering::Relaxed),
            self.handle_read_ns.load(Ordering::Relaxed),
            self.privilege_ns.load(Ordering::Relaxed),
            self.attach_tree_ns.load(Ordering::Relaxed),
        ]
    }

    pub(super) fn record_force(&self, started: Option<core::time::Duration>) {
        if let Some(started) = started {
            self.force_ns.fetch_add(
                kernel::tsc::read().saturating_sub(started).as_nanos() as u64,
                Ordering::Relaxed,
            );
        }
    }

    pub(super) fn record_call_time(&self, name: &[u8], started: Option<core::time::Duration>) {
        let Some(started) = started else {
            return;
        };
        let elapsed = kernel::tsc::read().saturating_sub(started);
        let field = match name {
            b"get_tree_element" => &self.tree_get_ns,
            b"unsound_handle_bytes" => &self.handle_read_ns,
            b"is_privileged" => &self.privilege_ns,
            b"get_tree" => &self.attach_tree_ns,
            _ => return,
        };
        field.fetch_add(elapsed.as_nanos() as u64, Ordering::Relaxed);
    }

    pub(super) fn capture(
        &self,
        mut function: Function,
        used: usize,
        reply: usize,
        attachments: Tuple,
        combination: Tree,
    ) -> Tree {
        self.captures.fetch_add(1, Ordering::Relaxed);
        handles::insert(&mut function, reply, Blob::literal(b"").unwrap().into());
        let mut addresses = Vec::new();
        let mut records = Vec::new();
        for record in attachments {
            let mut record: Tuple = record.try_into().expect("invalid attachment");
            let address: kernel::prelude::Word = record.take(0).try_into().unwrap();
            let backing = record.take(1);
            let length: kernel::prelude::Word = record.take(2).try_into().unwrap();
            let bytes: kernel::prelude::Word = record.take(3).try_into().unwrap();
            let writable: kernel::prelude::Word = record.take(4).try_into().unwrap();
            let handle = match backing {
                Value::Blob(reference) => {
                    handles::resolve(&mut function, unpack_reference(&reference))
                }
                Value::Null(_) => {
                    let backing = handles::backing(
                        &mut function,
                        address.read() as usize,
                        bytes.read() as usize,
                    );
                    unsafe { self.add_tree_backing(backing) }.into()
                }
                _ => panic!("invalid table backing"),
            };
            let address = address.read() as usize;
            if address < handles::HANDLE_BASE {
                addresses.push(address);
            }
            records.push(
                self.add_tree(&[
                    self.add_blob(&(address as u64).to_le_bytes()).into(),
                    handle,
                    self.add_blob(&length.read().to_le_bytes()).into(),
                    self.add_blob(&writable.read().to_le_bytes()).into(),
                ])
                .into(),
            );
        }
        let attachments = self.add_tree(&records);
        let arena = handles::backing(&mut function, handles::HANDLE_BASE, used);
        let arena = unsafe { self.add_tree_backing(arena) };
        addresses.push(handles::HANDLE_BASE);
        let comparable = arena.as_handle().is_comparable()
            && attachments.as_handle().is_comparable()
            && combination.as_handle().is_comparable();
        let value = arena.as_handle().is_value()
            && attachments.as_handle().is_value()
            && combination.as_handle().is_value();
        let resident = Arc::new(KMutex::new(Some(function)));
        let saved = resident.clone();
        let state = unsafe {
            self.inner
                .as_ref()
                .defer_tree(5, comparable, value, move |storage| {
                    let mut checkpoint = saved.lock().take().expect("missing native checkpoint");
                    for address in addresses {
                        for offset in (0..1usize << 32).step_by(1 << 30) {
                            assert!(
                                checkpoint
                                    .inner_mut()
                                    .arca_mut()
                                    .unwrap()
                                    .mappings_mut()
                                    .map(address + offset, kernel::prelude::Entry::Null(1 << 30))
                                    .is_ok()
                            );
                        }
                    }
                    let snapshot = checkpoint::encode(storage, checkpoint.into());
                    let fields: [Handle; 5] = [
                        snapshot,
                        arena.into(),
                        attachments.into(),
                        storage.add_blob(&(reply as u64).to_le_bytes()).into(),
                        combination.into(),
                    ];
                    let bytes = core::slice::from_raw_parts(
                        fields.as_ptr().cast(),
                        core::mem::size_of_val(&fields),
                    );
                    ImmutableBytes::new(bytes)
                })
        }
        .into_ref();
        let author = self.add_tree(&[]);
        let tree = self
            .add_tree(&[
                author.into(),
                state.into(),
                Blob::literal(b"Continuation").unwrap().into(),
            ])
            .tag_descriptor();
        self.continuations
            .lock()
            .insert(*tree.as_handle().as_bytes(), (resident, reply, combination));
        tree
    }

    pub(super) fn continuation(&self, tree: Tree) -> Option<(Function, usize, Tree)> {
        let resident = self
            .continuations
            .lock()
            .remove(tree.as_handle().as_bytes());
        if let Some((resident, reply, combination)) = resident {
            let mut saved = resident.lock();
            if let Some(function) = saved.take() {
                *saved = Some(function.clone());
                return Some((function, reply, combination));
            }
        }
        if !tree.is_tag() || tree.len() != 3 {
            return None;
        }
        let fields = self.inner.get_tree(tree).ok()?;
        if !trust::is_kernel_author(fields[0])
            || !fields[2].same_encoding(&Blob::literal(b"Continuation").unwrap().into())
        {
            return None;
        }
        let payload = Ref::try_from(fields[1]).ok()?.object_descriptor();
        let payload = self.get_tree(Tree::try_from(payload.handle()).ok()?).ok()?;
        assert_eq!(payload.len(), 5);
        let mut function: Function = self
            .decode(payload[0])
            .try_into()
            .expect("invalid attested checkpoint");
        let arena = self
            .get_tree_backing(Tree::try_from(payload[1]).unwrap())
            .unwrap();
        handles::map_backing(&mut function, handles::HANDLE_BASE, &arena, true);
        let attachments = self.get_tree(Tree::try_from(payload[2]).unwrap()).unwrap();
        for attachment in attachments {
            let record = self.get_tree(Tree::try_from(attachment).unwrap()).unwrap();
            let address = self.integer(record[0]) as usize;
            let length = self.integer(record[2]) as usize;
            let backing = match Object::try_from(record[1]).unwrap().view() {
                ObjectView::Blob(blob) => self.get_blob_backing(*blob).unwrap(),
                ObjectView::Tree(tree) => self.get_tree_backing(*tree).unwrap(),
            };
            let writable = self.integer(record[3]) != 0;
            handles::map_backing(&mut function, address, &backing, writable);
            for offset in (backing.len().next_multiple_of(4096)..length).step_by(4096) {
                assert!(
                    function
                        .inner_mut()
                        .arca_mut()
                        .unwrap()
                        .mappings_mut()
                        .map(
                            address + offset,
                            if writable {
                                kernel::prelude::Entry::RWPage(kernel::prelude::Page::new(4096))
                            } else {
                                kernel::prelude::Entry::ROPage(kernel::prelude::Page::new(4096))
                            }
                        )
                        .is_ok()
                );
            }
        }
        Some((
            function,
            self.integer(payload[3]) as usize,
            Tree::try_from(payload[4]).unwrap(),
        ))
    }

    pub(super) fn integer(&self, value: Handle) -> u64 {
        let bytes = self.get_blob(Blob::try_from(value).unwrap()).unwrap();
        u64::from_le_bytes(bytes.as_ref().try_into().unwrap())
    }
}

impl<S: Storage> Storage for Store<S> {
    fn identify(&self, bytes: &[u8; HANDLE_SIZE]) -> Result<Handle, StorageError> {
        self.inner.identify(bytes)
    }

    fn load(&self, handle: Handle) {
        self.inner.load(handle)
    }

    fn has_blob(&self, blob: Blob) -> bool {
        self.inner.has_blob(blob)
    }

    fn has_tree(&self, tree: Tree) -> bool {
        self.inner.has_tree(tree)
    }

    fn canonicalize(&self, handle: Handle) -> Result<CanonicalHandle, StorageError> {
        self.inner.canonicalize(handle)
    }

    fn add_blob(&self, bytes: &[u8]) -> Blob {
        self.inner.add_blob(bytes)
    }
    fn add_blob_backing(&self, data: ImmutableBytes) -> Blob {
        self.inner.add_blob_backing(data)
    }
    fn add_tree(&self, children: &[Handle]) -> Tree {
        self.inner.add_tree(children)
    }
    unsafe fn add_tree_backing(&self, data: ImmutableBytes) -> Tree {
        unsafe { self.inner.add_tree_backing(data) }
    }
    fn get_blob(&self, blob: Blob) -> Result<Box<[u8]>, StorageError> {
        self.inner.get_blob(blob)
    }
    fn get_blob_backing(&self, blob: Blob) -> Result<ImmutableBytes, StorageError> {
        self.inner.get_blob_backing(blob)
    }
    fn get_tree(&self, tree: Tree) -> Result<Box<[Handle]>, StorageError> {
        self.inner.get_tree(tree)
    }
    fn get_tree_backing(&self, tree: Tree) -> Result<ImmutableBytes, StorageError> {
        self.inner.get_tree_backing(tree)
    }
    fn canonicalize_blob(&self, blob: Blob) -> Result<CanonicalHandle, StorageError> {
        self.inner.canonicalize_blob(blob)
    }
    fn canonicalize_tree(&self, tree: Tree) -> Result<CanonicalHandle, StorageError> {
        self.inner.canonicalize_tree(tree)
    }
}
