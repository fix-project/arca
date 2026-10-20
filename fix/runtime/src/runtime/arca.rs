extern crate alloc;

use crate::handle::*;
use crate::runtime::Runtime;
use crate::storage::Storage;
use crate::storage::memory::MemoryStorage;
use crate::storage::repository::RepositoryStorage;
use alloc::sync::Arc;
pub mod trust;
use kernel::prelude::{Blob as ArcaBlob, Function, Tuple, Value, Vec};
#[cfg(debug_assertions)]
use kernel::println;

#[derive(Debug)]
pub struct FixOnArca<S = MemoryStorage> {
    storage: S,
    trust: trust::Trust,
    memoization: Option<Arc<RepositoryStorage>>,
}

impl Default for FixOnArca {
    fn default() -> Self {
        Self::new(MemoryStorage::default())
    }
}

impl<S: Storage> Runtime for FixOnArca<S> {
    fn storage(&self) -> &dyn Storage {
        &self.storage
    }

    fn execute(&self, combination: Tree) -> Tree {
        assert!(
            combination.as_handle().is_value(),
            "application requires an evaluated combination"
        );
        if let Some(cache) = &self.memoization {
            let apply = self.storage.add_blob(&2u64.to_le_bytes());
            let canonical = self.storage.canonicalize(combination.into()).unwrap();
            for coupon in cache
                .repository()
                .claims(&[apply.into(), canonical.into()])
                .unwrap()
            {
                let fields = cache.get_tree(coupon).unwrap();
                if fields.len() == 2
                    && let Ok(authors) = Tree::try_from(fields[0])
                    && authors.is_empty()
                {
                    return coupon;
                }
            }
        }
        let result = self.run_combination(combination, false);
        let coupon = self.storage.certificate(2, combination.into(), result);
        if let Some(cache) = &self.memoization {
            cache.save(coupon.into()).unwrap();
        }
        coupon
    }
}

impl<S: Storage> FixOnArca<S> {
    pub fn new(storage: S) -> Self {
        Self {
            storage,
            trust: trust::Trust::default(),
            memoization: None,
        }
    }

    pub fn with_trust(mut self, trust: trust::Trust) -> Self {
        self.trust = trust;
        self
    }

    /// Runs outside deterministic Fix semantics and must not receive an application coupon.
    pub fn execute_privileged(&self, combination: Tree) -> Handle {
        self.run_combination(combination, true)
    }
}

impl FixOnArca<Arc<RepositoryStorage>> {
    pub fn memoized(storage: Arc<RepositoryStorage>) -> Self {
        let mut runtime = Self::new(storage.clone());
        runtime.memoization = Some(storage);
        runtime
    }
}

impl<S: Storage> FixOnArca<S> {
    fn run_combination(&self, combination: Tree, privileged: bool) -> Handle {
        #[cfg(debug_assertions)]
        println!("applying   {}", Handle::from(combination));
        let contents = self.storage.get_tree(combination).unwrap();
        let mut procedure = *contents.get(1).expect("missing callable");
        let executable = loop {
            let tree = Tree::try_from(procedure).expect("callable must resolve to a Runnable Tag");
            procedure = *self
                .storage
                .get_tree(tree)
                .unwrap()
                .get(1)
                .expect("missing callable");
            if Blob::try_from(procedure).is_ok() {
                break self
                    .trust
                    .runnable(&self.storage, tree)
                    .unwrap_or_else(|error| panic!("{error}"));
            }
        };
        let elf = self.storage.get_blob(executable).unwrap();
        let f: Function = common::elfloader::load_elf(&elf).unwrap();
        drop(elf);
        let f = f.apply(pack_handle(combination));
        self.run(f, combination, privileged)
    }

    fn run(&self, mut f: Function, combination: Tree, privileged: bool) -> Handle {
        loop {
            let result = f.force();
            if let Value::Blob(b) = result {
                return unpack_handle(&b);
            } else {
                let Value::Function(g) = result else {
                    panic!("expected Fix program to return a handle or an effect")
                };
                let data = g.into_inner().read();
                let Value::Tuple(mut data) = data else {
                    unreachable!()
                };
                let t: ArcaBlob = data.take(0).try_into().unwrap();
                assert_eq!(&*t, b"Symbolic");
                let effect: ArcaBlob = data.take(1).try_into().unwrap();
                let args: Tuple = data.take(2).try_into().unwrap();
                let mut args: Vec<Value> = args.into_iter().collect();
                let Some(Value::Function(k)) = args.pop() else {
                    panic!("unexpected non-effect return");
                };

                f = self
                    .handle_effect(&effect, args, k, combination, privileged)
                    .unwrap_or_else(|| todo!("handle effect {:?}", &*effect));
            }
        }
    }
    pub(crate) fn handle_effect(
        &self,
        effect: &[u8],
        mut args: Vec<Value>,
        k: Function,
        combination: Tree,
        privileged: bool,
    ) -> Option<Function> {
        Some(match effect {
            b"is_privileged" => k.apply(kernel::prelude::Word::new(privileged as u64)),
            b"unsound_handle_bytes" => {
                assert!(privileged, "handle bytes require a privileged application");
                let Some(Value::Blob(value)) = args.pop() else {
                    panic!("missing handle")
                };
                let value = unpack_handle(&value);
                let bytes = self.storage.add_blob(value.as_bytes());
                k.apply(pack_handle(bytes))
            }
            b"equals" => {
                let Some(Value::Blob(rhs)) = args.pop() else {
                    panic!()
                };
                let Some(Value::Blob(lhs)) = args.pop() else {
                    panic!()
                };
                let equal = self
                    .storage
                    .equals(unpack_handle(&lhs), unpack_handle(&rhs))
                    .expect("equals: invalid or non-comparable operand");
                k.apply(kernel::prelude::Word::new(equal as u64))
            }
            b"create_blob_i32" => {
                let Some(Value::Word(w)) = args.pop() else {
                    panic!()
                };
                {
                    let value = self.storage.add_blob(&u32::to_le_bytes(w.read() as u32));
                    k.apply(pack_handle(value))
                }
            }
            b"create_blob_i64" => {
                let Some(Value::Word(w)) = args.pop() else {
                    panic!()
                };
                {
                    let value = self.storage.add_blob(&u64::to_le_bytes(w.read()));
                    k.apply(pack_handle(value))
                }
            }
            b"create_blob" => {
                let Some(Value::Blob(b)) = args.pop() else {
                    panic!()
                };
                {
                    let value = self.storage.add_blob(&b);
                    k.apply(pack_handle(value))
                }
            }
            b"create_tree" | b"create_tag" => {
                let tag = effect == b"create_tag";
                let depth = if tag {
                    let Some(Value::Word(depth)) = args.pop() else {
                        panic!("missing author depth")
                    };
                    depth.read() as usize
                } else {
                    0
                };
                let Some(Value::Word(length)) = args.pop() else {
                    panic!("missing tree length")
                };
                let tables = match args.pop().expect("missing tree backing") {
                    Value::Table(table) => kernel::prelude::vec![table],
                    Value::Tuple(tables) => tables
                        .into_iter()
                        .map(|table| table.try_into().expect("expected backing table"))
                        .collect(),
                    _ => panic!("invalid tree backing"),
                };
                let mut data =
                    crate::storage::ImmutableBytes::from_tables(tables, length.read() as usize);
                if tag {
                    let authors = Tree::try_from(read_backing(&data, 0))
                        .expect("tag authors must be a Tree Object");
                    let mut author: Handle = combination.into();
                    for _ in 0..depth {
                        let tree =
                            Tree::try_from(author).expect("author depth exceeds callable path");
                        author = read_backing(&self.storage.get_tree_backing(tree).unwrap(), 1);
                    }
                    let extra = self.storage.get_tree_backing(authors).unwrap();
                    let duplicate = author.is_comparable()
                        && (0..authors.len()).any(|index| {
                            self.storage.equals(read_backing(&extra, index), author) == Ok(true)
                        });
                    if !duplicate {
                        let length = extra.len();
                        let authors = unsafe {
                            self.storage
                                .add_tree_backing(extra.with_bytes(length, author.as_bytes()))
                        };
                        data = data.with_bytes(0, authors.as_handle().as_bytes());
                    }
                }
                let value = unsafe { self.storage.add_tree_backing(data) };
                let value = if tag { value.tag_descriptor() } else { value };
                k.apply(pack_handle(value))
            }
            b"get_blob" => {
                let Some(Value::Blob(b)) = args.pop() else {
                    panic!()
                };
                let name = Blob::try_from(unpack_handle(&b)).expect("expected blob object");
                k.apply(self.storage.get_blob_backing(name).unwrap().mapped_value())
            }
            b"get_tree_element" => {
                let Some(Value::Word(index)) = args.pop() else {
                    panic!("missing Tree index")
                };
                let Some(Value::Blob(value)) = args.pop() else {
                    panic!("missing Tree handle")
                };
                let tree = Tree::try_from(unpack_handle(&value)).expect("expected Tree Object");
                let index = index.read() as usize;
                assert!(index < tree.len(), "Tree index out of bounds");
                let child = read_backing(&self.storage.get_tree_backing(tree).unwrap(), index);
                k.apply(pack_handle(child))
            }
            b"get_tree" => {
                let Some(Value::Blob(b)) = args.pop() else {
                    panic!()
                };
                let name = Tree::try_from(unpack_handle(&b)).expect("expected tree object");
                let backing = self.storage.get_tree_backing(name).unwrap();
                k.apply(backing.mapped_value())
            }
            _ => return None,
        })
    }
}

pub(crate) fn pack_handle(handle: impl Into<Handle>) -> ArcaBlob {
    let raw = handle.into().into_bytes();
    ArcaBlob::new(raw)
}

//needed since we want to error out instead of
pub(crate) fn decode_handle(blob: &ArcaBlob) -> Result<Handle, &'static str> {
    if blob.len() != HANDLE_SIZE {
        return Err("invalid handle length");
    }
    let mut buf = [0u8; HANDLE_SIZE];
    if blob.read(0, &mut buf) != HANDLE_SIZE {
        return Err("Failed to parse Arca Blob to Fix Handle");
    }

    unsafe { Handle::parse(buf) }.map_err(|_| "invalid Fix handle")
}

fn unpack_handle(blob: &ArcaBlob) -> Handle {
    assert_eq!(blob.len(), HANDLE_SIZE, "invalid handle length");
    let mut buf = [0u8; 32];
    if blob.read(0, &mut buf) != 32 {
        panic!("Failed to parse Arca Blob to Fix Handle")
    }
    unsafe { Handle::parse(buf) }.expect("invalid Fix handle")
}

fn read_backing(backing: &crate::storage::ImmutableBytes, index: usize) -> Handle {
    let (page, offset) = backing.page(index * HANDLE_SIZE);
    let mut bytes = [0; HANDLE_SIZE];
    assert_eq!(page.read(offset, &mut bytes), HANDLE_SIZE);
    unsafe { Handle::parse(bytes) }.expect("invalid stored handle")
}
