use core::marker::PhantomData;

use alloc::collections::btree_map::BTreeMap;
use async_trait::async_trait;
use enumflags2::BitFlags;
use kernel::prelude::*;
use ninep::*;

use crate::*;

pub struct DevFS;

#[async_trait]
impl VirtualClosedDir for DevFS {
    fn init(&self) -> BTreeMap<String, VClosedNode> {
        let mut map = BTreeMap::new();
        map.insert(
            "null".to_string(),
            VClosedNode::File(Dev::<Null>(PhantomData).into()),
        );
        map.insert(
            "zero".to_string(),
            VClosedNode::File(Dev::<Zero>(PhantomData).into()),
        );
        map.insert(
            "cons".to_string(),
            VClosedNode::File(Dev::<Cons>(PhantomData).into()),
        );
        map
    }

    async fn open(&self, _: Access) -> Result<Box<dyn VirtualOpenDir>> {
        Ok(Box::new(DevFS))
    }

    async fn mkdir(&mut self, _: &str, _: BitFlags<Perm>, _: BitFlags<Flag>) -> Result<VClosedDir> {
        Err(Error::PermissionDenied)
    }

    async fn create(
        &mut self,
        _: &str,
        _: BitFlags<Perm>,
        _: BitFlags<Flag>,
    ) -> Result<VClosedFile> {
        Err(Error::PermissionDenied)
    }
}

#[async_trait]
impl VirtualOpenDir for DevFS {}

#[derive(Copy, Clone)]
struct Null;

#[derive(Copy, Clone)]
struct Zero;

#[derive(Copy, Clone)]
struct Cons;

#[derive(Copy, Clone)]
struct Dev<T>(PhantomData<T>);

#[async_trait]
impl<T: 'static> VirtualClosedFile for Dev<T>
where
    Dev<T>: VirtualOpenFile + Copy,
{
    async fn open(&self, _: Access) -> Result<Box<dyn VirtualOpenFile>> {
        Ok(Box::new(*self))
    }
}

#[async_trait]
impl VirtualOpenFile for Dev<Null> {
    async fn read(&self, _: usize, _: &mut [u8]) -> Result<usize> {
        Ok(0)
    }

    async fn write(&mut self, _: usize, buf: &[u8]) -> Result<usize> {
        Ok(buf.len())
    }
}

#[async_trait]
impl VirtualOpenFile for Dev<Zero> {
    async fn read(&self, _: usize, buf: &mut [u8]) -> Result<usize> {
        buf.fill(0);
        Ok(buf.len())
    }

    async fn write(&mut self, _: usize, buf: &[u8]) -> Result<usize> {
        Ok(buf.len())
    }
}

#[async_trait]
impl VirtualOpenFile for Dev<Cons> {
    async fn read(&self, _: usize, buf: &mut [u8]) -> Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        };
        let mut cons = kernel::debugcon::CONSOLE.lock();
        buf[0] = cons.read_byte();
        Ok(1)
    }

    async fn write(&mut self, _: usize, buf: &[u8]) -> Result<usize> {
        let mut cons = kernel::debugcon::CONSOLE.lock();
        cons.write(buf);
        Ok(buf.len())
    }
}
