use crate::handle::*;
use crate::runtime::Runtime;
use crate::storage::Storage;
use crate::storage::memory::MemoryStorage;
use kernel::prelude::{Blob as ArcaBlob, Function, Tuple, Value, Vec};
use kernel::println;

#[derive(Debug, Default)]
pub struct FixOnArca {
    storage: MemoryStorage,
}

impl Runtime for FixOnArca {
    fn storage(&self) -> &dyn Storage {
        &self.storage
    }

    fn execute(&self, combination: Tree) -> Handle {
        println!("applying   {}", Handle::from(combination));
        let contents = self.storage().get_tree(combination).unwrap();
        let procedure = contents.first().expect("empty combination");
        let elf = self
            .storage()
            .get_blob(Blob::try_from(*procedure).expect("procedure must be a blob object"))
            .unwrap();
        let f: Function = common::elfloader::load_elf(&elf).unwrap();
        let blob = pack_handle(combination);
        let f = f.apply(blob);
        self.run(f)
    }
}

impl FixOnArca {
    fn run(&self, mut f: Function) -> Handle {
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

                f = match &*effect {
                    b"equals" => {
                        let Some(Value::Blob(rhs)) = args.pop() else {
                            panic!()
                        };
                        let Some(Value::Blob(lhs)) = args.pop() else {
                            panic!()
                        };
                        let equal = self
                            .storage()
                            .equals(unpack_handle(&lhs), unpack_handle(&rhs))
                            .expect("equals: invalid or non-Eq operand");
                        k.apply(kernel::prelude::Word::new(equal as u64))
                    }
                    b"create_blob_i32" => {
                        let Some(Value::Word(w)) = args.pop() else {
                            panic!()
                        };
                        k.apply(pack_handle(
                            self.storage().add_blob(&u32::to_le_bytes(w.read() as u32)),
                        ))
                    }
                    b"create_blob_i64" => {
                        let Some(Value::Word(w)) = args.pop() else {
                            panic!()
                        };
                        k.apply(pack_handle(
                            self.storage().add_blob(&u64::to_le_bytes(w.read())),
                        ))
                    }
                    b"create_blob" => {
                        let Some(Value::Blob(b)) = args.pop() else {
                            panic!()
                        };
                        k.apply(pack_handle(self.storage().add_blob(&b)))
                    }
                    b"create_tree" => {
                        let Some(Value::Blob(t)) = args.pop() else {
                            panic!()
                        };
                        let mut tree = Vec::new();
                        assert!(
                            t.len().is_multiple_of(HANDLE_SIZE),
                            "invalid tree payload length"
                        );
                        for handle in t.as_chunks::<HANDLE_SIZE>().0 {
                            tree.push(
                                unsafe { Handle::parse(*handle) }.expect("invalid tree entry"),
                            );
                        }
                        k.apply(pack_handle(self.storage().add_tree(&tree)))
                    }
                    b"get_blob" => {
                        let Some(Value::Blob(b)) = args.pop() else {
                            panic!()
                        };
                        let name = Blob::try_from(unpack_handle(&b)).expect("expected blob object");
                        k.apply(self.storage().get_blob_backing(name).unwrap().value())
                    }
                    b"get_tree" => {
                        let Some(Value::Blob(b)) = args.pop() else {
                            panic!()
                        };
                        let name = Tree::try_from(unpack_handle(&b)).expect("expected tree object");
                        k.apply(self.storage().get_tree_backing(name).unwrap().value())
                    }
                    _ => {
                        todo!("handle effect {:?}", &*effect);
                    }
                };
            }
        }
    }
}

fn pack_handle(handle: impl Into<Handle>) -> ArcaBlob {
    let raw = handle.into().into_bytes();
    ArcaBlob::new(raw)
}

fn unpack_handle(blob: &ArcaBlob) -> Handle {
    assert_eq!(blob.len(), HANDLE_SIZE, "invalid handle length");
    let mut buf = [0u8; 32];
    if blob.read(0, &mut buf) != 32 {
        panic!("Failed to parse Arca Blob to Fix Handle")
    }
    unsafe { Handle::parse(buf) }.expect("invalid Fix handle")
}
