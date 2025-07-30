use async_trait::async_trait;
use kernel::prelude::*;

use crate::*;

pub struct DevFS;

#[async_trait]
impl DirLike for DevFS {
    async fn open(&mut self, _config: Flags) -> Result<()> {
        Ok(())
    }

    async fn create(&mut self, _name: &str, _config: Mode) -> Result<Object> {
        Err(Error::PermissionDenied)?
    }

    async fn read(&self, _offset: usize, _count: usize) -> Result<Vec<DirEnt>> {
        Ok(vec![
            DirEnt {
                name: "null".into(),
                ..Default::default()
            },
            DirEnt {
                name: "zero".into(),
                ..Default::default()
            },
            DirEnt {
                name: "console".into(),
                ..Default::default()
            },
        ])
    }

    async fn walk(&self, path: &Path) -> Result<Object> {
        let result = match path.as_ref() {
            "null" => Object::File(Box::new(Null)),
            "zero" => Object::File(Box::new(Zero)),
            "cons" => Object::File(Box::new(Cons)),
            _ => Err(Error::NoSuchFileOrDirectory)?,
        };
        Ok(result)
    }

    async fn close(self) -> Result<()> {
        Ok(())
    }

    async fn remove(self) -> Result<()> {
        Err(Error::PermissionDenied)?
    }

    async fn stat(&self) -> Result<Stat> {
        todo!();
    }
}

#[derive(Copy, Clone)]
pub struct Cons;

#[async_trait]
impl FileLike for Cons {
    async fn open(&mut self, _config: Flags) -> Result<()> {
        Ok(())
    }

    async fn read(&self, _offset: usize, buf: &mut [u8]) -> Result<usize> {
        if buf.len() > 0 {
            let mut console = kernel::debugcon::CONSOLE.lock();
            buf[0] = console.read_byte();
        }
        Ok(if buf.len() > 0 { 1 } else { 0 })
    }

    async fn write(&mut self, _offset: usize, buf: &[u8]) -> Result<usize> {
        let mut console = kernel::debugcon::CONSOLE.lock();
        console.write(buf);
        Ok(buf.len())
    }

    async fn close(self) -> Result<()> {
        Ok(())
    }

    async fn remove(self) -> Result<()> {
        Err(Error::OperationNotPermitted)
    }

    async fn stat(&self) -> Result<Stat> {
        todo!();
    }

    async fn duplicate(&self) -> Result<File> {
        Ok(Box::new(*self))
    }
}

#[derive(Copy, Clone)]
pub struct Zero;

#[async_trait]
impl FileLike for Zero {
    async fn open(&mut self, _config: Flags) -> Result<()> {
        Ok(())
    }

    async fn read(&self, _offset: usize, buf: &mut [u8]) -> Result<usize> {
        buf.fill(0);
        Ok(buf.len())
    }

    async fn write(&mut self, _offset: usize, buf: &[u8]) -> Result<usize> {
        Ok(buf.len())
    }

    async fn close(self) -> Result<()> {
        Ok(())
    }

    async fn remove(self) -> Result<()> {
        Err(Error::OperationNotPermitted)
    }

    async fn stat(&self) -> Result<Stat> {
        todo!();
    }

    async fn duplicate(&self) -> Result<File> {
        Ok(Box::new(*self))
    }
}

#[derive(Copy, Clone)]
pub struct Null;

#[async_trait]
impl FileLike for Null {
    async fn open(&mut self, _config: Flags) -> Result<()> {
        Ok(())
    }

    async fn read(&self, _offset: usize, _buf: &mut [u8]) -> Result<usize> {
        Ok(0)
    }

    async fn write(&mut self, _offset: usize, buf: &[u8]) -> Result<usize> {
        Ok(buf.len())
    }

    async fn close(self) -> Result<()> {
        Ok(())
    }

    async fn remove(self) -> Result<()> {
        Err(Error::OperationNotPermitted)
    }

    async fn stat(&self) -> Result<Stat> {
        todo!();
    }

    async fn duplicate(&self) -> Result<File> {
        Ok(Box::new(*self))
    }
}
