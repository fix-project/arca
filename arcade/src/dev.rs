use futures::{StreamExt, stream::BoxStream};
use kernel::debugcon;
use vfs::{Create, Dir, DirEnt, ErrorKind, File, Object, Open, Result, SeekFrom};

#[derive(Default, Clone)]
pub struct DevFS {
    open: Open,
}

impl Dir for DevFS {
    async fn open(&self, name: &str, open: Open) -> Result<Object> {
        Ok(Object::File(
            match name {
                "null" => Dev {
                    open,
                    mode: Mode::Null,
                },
                "zero" => Dev {
                    open,
                    mode: Mode::Zero,
                },
                "cons" => Dev {
                    open,
                    mode: Mode::Cons,
                },
                _ => return Err(ErrorKind::NotFound.into()),
            }
            .boxed(),
        ))
    }

    async fn readdir(&self) -> Result<BoxStream<'_, Result<DirEnt>>> {
        if !self.open.contains(Open::Read) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        Ok(futures::stream::iter(
            [
                DirEnt {
                    name: "null".into(),
                    dir: false,
                },
                DirEnt {
                    name: "zero".into(),
                    dir: false,
                },
                DirEnt {
                    name: "cons".into(),
                    dir: false,
                },
            ]
            .map(Ok),
        )
        .boxed())
    }

    async fn create(&self, _: &str, _: Create, _: Open) -> Result<Object> {
        Err(ErrorKind::PermissionDenied.into())
    }

    async fn remove(&self, _: &str) -> Result<()> {
        Err(ErrorKind::PermissionDenied.into())
    }

    async fn dup(&self) -> Result<Self> {
        Ok(self.clone())
    }
}

#[derive(Default, Clone)]
struct Dev {
    open: Open,
    mode: Mode,
}

#[derive(Default, Clone)]
pub enum Mode {
    #[default]
    Null,
    Zero,
    Cons,
}

impl File for Dev {
    async fn read(&mut self, bytes: &mut [u8]) -> Result<usize> {
        if !self.open.contains(Open::Read) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        Ok(match self.mode {
            Mode::Null => 0,
            Mode::Zero => {
                bytes.fill(0);
                bytes.len()
            }
            Mode::Cons => {
                let mut cons = debugcon::CONSOLE.lock();
                if !bytes.is_empty() {
                    let b = cons.read_byte();
                    bytes[0] = b;
                    1
                } else {
                    0
                }
            }
        })
    }

    async fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        if !self.open.contains(Open::Write) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        Ok(match self.mode {
            Mode::Null => bytes.len(),
            Mode::Zero => bytes.len(),
            Mode::Cons => {
                let mut cons = debugcon::CONSOLE.lock();
                cons.write(bytes);
                bytes.len()
            }
        })
    }

    async fn seek(&mut self, _: SeekFrom) -> Result<usize> {
        Ok(0)
    }

    async fn dup(&self) -> Result<Self> {
        Ok(self.clone())
    }
}
