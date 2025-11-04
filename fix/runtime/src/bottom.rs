#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

use crate::{
    data::{BlobData, RawData, TreeData},
    fixruntime::FixRuntime,
    runtime::{DeterministicEquivRuntime, Executor},
};

use core::simd::u8x32;

use arca::Runtime;
use fixhandle::rawhandle::{BitPack, FixHandle};
use kernel::prelude::vec;
use kernel::{
    prelude::vec::Vec,
    types::{Blob as ArcaBlob, Function, Tuple, Value},
};

#[derive(Debug)]
pub enum Error {
    FixRuntimeError,
}

fn pack_handle(handle: &FixHandle) -> ArcaBlob {
    let raw = handle.pack();
    Runtime::create_blob(raw.as_array())
}

fn unpack_handle(blob: &ArcaBlob) -> FixHandle {
    let mut buf = [0u8; 32];
    if Runtime::read_blob(blob, 0, &mut buf) != 32 {
        panic!("Failed to parse Arca Blob to Fix Handle")
    }
    FixHandle::unpack(u8x32::from_array(buf))
}

pub struct FixShellBottom<'a> {
    parent: &'a mut FixRuntime<'a>,
}

impl<'a> DeterministicEquivRuntime for FixShellBottom<'a> {
    type BlobData = BlobData;
    type TreeData = TreeData;
    type Handle = ArcaBlob;
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

impl<'a> FixShellBottom<'a> {
    fn run(&mut self, mut f: Function) -> FixHandle {
        loop {
            let result = f.force();
            if let Value::Blob(b) = result {
                return unpack_handle(&b);
            } else {
                let Value::Function(g) = result else { panic!() };
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
                    panic!()
                };

                f = match &*effect {
                    b"create_blob_i64" => {
                        let Some(Value::Word(w)) = args.pop() else {
                            panic!()
                        };
                        k.apply(self.create_blob_i64(w.read()))
                    }
                    b"create_blob" => {
                        let Some(Value::Table(t)) = args.pop() else {
                            panic!()
                        };
                        let Some(Value::Word(w)) = args.pop() else {
                            panic!()
                        };
                        k.apply(self.create_blob(BlobData::new(t, w.read() as usize)))
                    }
                    b"create_tree" => {
                        let Some(Value::Table(t)) = args.pop() else {
                            panic!()
                        };
                        let Some(Value::Word(w)) = args.pop() else {
                            panic!()
                        };
                        k.apply(self.create_tree(TreeData::new(t, w.read() as usize)))
                    }
                    b"get_blob" => {
                        let Some(Value::Blob(b)) = args.pop() else {
                            panic!()
                        };
                        let t: RawData = self.get_blob(&b).expect("").into();
                        k.apply(Value::Table(t.into()))
                    }
                    b"get_tree" => {
                        let Some(Value::Blob(b)) = args.pop() else {
                            panic!()
                        };
                        let t: RawData = self.get_tree(&b).expect("").into();
                        k.apply(Value::Table(t.into()))
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
                    _ => unreachable!(),
                };
            }
        }
    }
}

impl<'a> Executor for FixShellBottom<'a> {
    fn execute(&mut self, combination: &FixHandle) -> FixHandle {
        let tree = self.parent.get_tree(combination).unwrap();
        let function_handle = tree.get(1);
        let elf = self.parent.get_blob(&function_handle).unwrap();

        let mut buffer = vec![0u8; elf.len()];
        elf.get(&mut buffer);

        let f = common::elfloader::load_elf(&buffer).expect("Failed to load elf");
        let f = Runtime::apply_function(f, Value::from(pack_handle(combination)));

        self.run(f)
    }
}
