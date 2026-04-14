#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

use crate::{
    // data::{BlobData, RawData, TreeData},
    fixruntime::FixRuntime,
    runtime::{DeterministicEquivRuntime, Executor},
};

use arca::Runtime;
use fixhandle::rawhandle::{BitPack, FixHandle};
use kernel::{
    prelude::vec::Vec,
    types::{Blob, Function, Tuple, Value},
};

#[derive(Debug)]
pub enum Error {
    FixRuntimeError,
}

fn pack_handle(handle: &FixHandle) -> Blob {
    let raw = handle.pack();
    Runtime::create_blob(&raw)
}

fn unpack_handle(blob: &Blob) -> FixHandle {
    let mut buf = [0u8; 32];
    if Runtime::read_blob(blob, 0, &mut buf) != 32 {
        panic!("Failed to parse Arca Blob to Fix Handle")
    }
    FixHandle::unpack(buf)
}

pub struct FixShellBottom<'a, 'b> {
    pub parent: &'b mut FixRuntime<'a>,
}

impl<'a, 'b> DeterministicEquivRuntime for FixShellBottom<'a, 'b> {
    type BlobData = Blob;
    type TreeData = Tuple;
    type Handle = Blob;
    type Error = Error;

    fn create_blob_i64(&mut self, data: u64) -> Self::Handle {
        pack_handle(&self.parent.create_blob_i64(data))
    }

    fn create_blob(&mut self, data: Self::BlobData) -> Self::Handle {
        pack_handle(&self.parent.create_blob(data))
    }

    fn create_tree(&mut self, data: Self::TreeData) -> Self::Handle {
        pack_handle(&self.parent.create_tree(data))
    }

    fn get_blob(&self, handle: &Self::Handle) -> Result<Self::BlobData, Self::Error> {
        self.parent
            .get_blob(&unpack_handle(handle))
            .map_err(|_| Error::FixRuntimeError)
    }

    fn get_tree(&self, handle: &Self::Handle) -> Result<Self::TreeData, Self::Error> {
        self.parent
            .get_tree(&unpack_handle(handle))
            .map_err(|_| Error::FixRuntimeError)
    }

    fn is_blob(handle: &Self::Handle) -> bool {
        FixRuntime::is_blob(&unpack_handle(handle))
    }

    fn is_tree(handle: &Self::Handle) -> bool {
        FixRuntime::is_tree(&unpack_handle(handle))
    }
}

impl<'a, 'b> FixShellBottom<'a, 'b> {
    fn run(&mut self, mut f: Function) -> FixHandle {
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
                let t: Blob = data.take(0).try_into().unwrap();
                assert_eq!(&*t, b"Symbolic");
                let effect: Blob = data.take(1).try_into().unwrap();
                let args: Tuple = data.take(2).try_into().unwrap();
                let mut args: Vec<Value> = args.into_iter().collect();
                let Some(Value::Function(k)) = args.pop() else {
                    panic!("unexpected non-effect return");
                };

                f = match &*effect {
                    b"create_blob_i64" => {
                        let Some(Value::Word(w)) = args.pop() else {
                            panic!()
                        };
                        k.apply(self.create_blob_i64(w.read()))
                    }
                    b"create_blob" => {
                        let Some(Value::Blob(b)) = args.pop() else {
                            panic!()
                        };
                        k.apply(self.create_blob(b))
                    }
                    b"create_tree" => {
                        let Some(Value::Tuple(t)) = args.pop() else {
                            panic!()
                        };
                        k.apply(self.create_tree(t))
                    }
                    b"get_blob" => {
                        let Some(Value::Blob(b)) = args.pop() else {
                            panic!()
                        };
                        let b = self.get_blob(&b).expect("");
                        k.apply(b)
                    }
                    b"get_tree" => {
                        let Some(Value::Blob(b)) = args.pop() else {
                            panic!()
                        };
                        let t = self.get_tree(&b).expect("");
                        k.apply(t)
                    }
                    b"is_blob" => {
                        let Some(Value::Blob(b)) = args.pop() else {
                            panic!()
                        };
                        k.apply(Runtime::create_word(Self::is_blob(&b) as u64))
                    }
                    b"is_tree" => {
                        let Some(Value::Blob(b)) = args.pop() else {
                            panic!()
                        };
                        k.apply(Runtime::create_word(Self::is_tree(&b) as u64))
                    }
                    _ => {
                        log::info!("{:?}", &*effect);
                        unreachable!();
                    }
                };
            }
        }
    }
}

impl<'a, 'b> Executor for FixShellBottom<'a, 'b> {
    fn execute(&mut self, combination: &FixHandle) -> FixHandle {
        let tree = self.parent.get_tree(combination).unwrap();
        let function_handle = tree.get(1);
        let function_handle = Blob::try_from(function_handle).unwrap();
        let mut bytes = [0; 32];
        function_handle.read(0, &mut bytes);
        let function_handle = FixHandle::unpack(bytes);
        let elf = self.parent.get_blob(&function_handle).unwrap();

        let f = common::elfloader::load_elf(&elf).expect("Failed to load elf");
        let f = Runtime::apply_function(f, Value::from(pack_handle(combination)));

        self.run(f)
    }
}
