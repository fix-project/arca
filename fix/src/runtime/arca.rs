use kernel::println;
use crate::runtime::Runtime;
use crate::storage::Storage;
use crate::storage::memory::MemoryStorage;
use crate::handle::*;
use common::bitpack::BitPack;
use kernel::prelude::{Function, Blob as ArcaBlob, Tuple, Vec, Value};

#[derive(Debug, Default)]
pub struct FixOnArca {
    storage: MemoryStorage
}

impl Runtime for FixOnArca {
    fn storage(&self) -> &dyn Storage {
        &self.storage
    }

    fn execute(&self, combination: Tree) -> Handle {
        println!("applying   {}", Handle::from(combination));
        let contents = self.storage().get_tree(combination).unwrap();
        let procedure = contents.get(0).expect("empty combination");
        let elf = self.storage().get_blob(procedure.unwrap_object().unwrap_blob()).unwrap();
        let f: Function = common::elfloader::load_elf(&elf).unwrap();
        let blob = pack_handle(combination);
        let f = f.apply(blob);
        let result = self.run(f);
        result
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
                    b"create_blob_i32" => {
                        let Some(Value::Word(w)) = args.pop() else {
                            panic!()
                        };
                        k.apply(pack_handle(self.storage().add_blob(&u32::to_le_bytes(w.read() as u32))))
                    }
                    b"create_blob_i64" => {
                        let Some(Value::Word(w)) = args.pop() else {
                            panic!()
                        };
                        k.apply(pack_handle(self.storage().add_blob(&u64::to_le_bytes(w.read()))))
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
                        for handle in t.chunks(32) {
                            tree.push(Handle::unpack(handle.try_into().unwrap()));
                        }
                        k.apply(pack_handle(self.storage().add_tree(&tree)))
                    }
                    b"get_blob" => {
                        let Some(Value::Blob(b)) = args.pop() else {
                            panic!()
                        };
                        let b = self.storage().get_blob(unpack_handle(&b).unwrap_object().unwrap_blob()).unwrap();
                        k.apply(ArcaBlob::new(b))
                    }
                    b"get_tree" => {
                        let Some(Value::Blob(b)) = args.pop() else {
                            panic!()
                        };
                        let t = self.storage().get_tree(unpack_handle(&b).unwrap_object().unwrap_tree()).unwrap();
                        let mut tree = Vec::new();
                        for x in t {
                            tree.extend_from_slice(&Handle::pack(&x));
                        }
                        k.apply(ArcaBlob::new(tree))
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
    let raw = handle.into().pack();
    ArcaBlob::new(&raw)
}

fn unpack_handle(blob: &ArcaBlob) -> Handle {
    let mut buf = [0u8; 32];
    if blob.read(0, &mut buf) != 32 {
        panic!("Failed to parse Arca Blob to Fix Handle")
    }
    Handle::unpack(buf)
}
