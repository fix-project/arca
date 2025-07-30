use enumflags2::BitFlag;
use kernel::prelude::*;

use super::*;

#[derive(Default, Clone)]
struct FileData {
    bytes: Arc<RwLock<Vec<u8>>>,
    append_only: bool,
}

#[derive(Default, Clone)]
struct DirData {
    entries: Arc<RwLock<Vec<VDirEnt>>>,
}

#[derive(Default)]
pub struct VDir {
    data: DirData,
    access: Option<Access>,
    rclose: bool,
    perm: Perms,
    kind: BitFlags<Kind>,
}

#[derive(Default, Clone)]
pub struct VFile {
    data: FileData,
    access: Option<Access>,
    rclose: bool,
    perm: Perms,
    kind: BitFlags<Kind>,
}

impl VDir {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone)]
enum VDirEnt {
    File {
        name: String,
        perm: Perms,
        data: FileData,
    },
    Dir {
        name: String,
        perm: Perms,
        data: DirData,
    },
}

impl VDirEnt {
    pub fn name(&self) -> &str {
        match self {
            VDirEnt::File {
                name,
                perm: _,
                data: _,
            } => name,
            VDirEnt::Dir {
                name,
                perm: _,
                data: _,
            } => name,
        }
    }

    pub fn perm(&self) -> Perms {
        *match self {
            VDirEnt::File {
                name: _,
                perm,
                data: _,
            } => perm,
            VDirEnt::Dir {
                name: _,
                perm,
                data: _,
            } => perm,
        }
    }

    pub fn append_only(&self) -> bool {
        match self {
            VDirEnt::File {
                name: _,
                perm: _,
                data,
            } => data.append_only,
            VDirEnt::Dir {
                name: _,
                perm: _,
                data: _,
            } => false,
        }
    }

    pub fn file(&self) -> bool {
        match self {
            VDirEnt::File {
                name: _,
                perm: _,
                data: _,
            } => true,
            VDirEnt::Dir {
                name: _,
                perm: _,
                data: _,
            } => false,
        }
    }

    pub fn dir(&self) -> bool {
        match self {
            VDirEnt::File {
                name: _,
                perm: _,
                data: _,
            } => false,
            VDirEnt::Dir {
                name: _,
                perm: _,
                data: _,
            } => true,
        }
    }
}

#[async_trait]
impl DirLike for VDir {
    async fn open(&mut self, config: Flags) -> Result<()> {
        self.rclose = config.rclose;
        self.access = Some(config.access);
        Ok(())
    }

    async fn create(&mut self, mut name: &str, config: Mode) -> Result<Object> {
        if !self.access.ok_or(Error::BadFileDescriptor)?.write() {
            Err(Error::PermissionDenied)?
        }
        if self
            .data
            .entries
            .read()
            .iter()
            .filter(|&x| x.name() == name)
            .count()
            != 0
        {
            Err(Error::FileExists)?
        };
        while name.starts_with("/") {
            name = name.get(1..).ok_or(Error::FileExists)?;
        }
        if config.kind.contains(Kind::Directory) {
            if !(config.kind & !Kind::Directory).is_empty() {
                Err(Error::OperationNotPermitted)?;
            }
            let data = DirData {
                entries: Arc::default(),
            };
            self.data.entries.write().push(VDirEnt::Dir {
                name: name.to_string(),
                perm: config.perm,
                data: data.clone(),
            });
            let file = VDir {
                data,
                access: Some(config.open.access),
                rclose: config.open.rclose,
                perm: config.perm,
                kind: config.kind,
            };
            Ok(Object::Dir(Box::new(file)))
        } else {
            if !(config.kind & !Kind::AppendOnly).is_empty() {
                Err(Error::OperationNotPermitted)?;
            }
            let data = FileData {
                bytes: Arc::default(),
                append_only: config.kind.contains(Kind::AppendOnly),
            };
            self.data.entries.write().push(VDirEnt::File {
                name: name.to_string(),
                perm: config.perm,
                data: data.clone(),
            });
            let file = VFile {
                data,
                access: Some(config.open.access),
                rclose: config.open.rclose,
                perm: config.perm,
                kind: config.kind,
            };
            Ok(Object::File(Box::new(file)))
        }
    }

    async fn read(&self, offset: usize, count: usize) -> Result<Vec<DirEnt>> {
        if !self.access.ok_or(Error::BadFileDescriptor)?.read() {
            return Err(Error::PermissionDenied).into();
        }
        let entries = self.data.entries.read();
        if offset >= entries.len() {
            return Ok(vec![]);
        }
        let n = core::cmp::min(count, entries.len() - offset);
        let mut results = vec![];
        for entry in &entries[offset..offset + n] {
            results.push(entry.clone().into());
        }
        Ok(results)
    }

    async fn walk(&self, path: &Path) -> Result<Object> {
        if !self.access.ok_or(Error::BadFileDescriptor)?.read() {
            return Err(Error::PermissionDenied).into();
        }
        if path.is_empty() {
            return Ok(Object::Dir(Box::new(VDir {
                data: self.data.clone(),
                access: self.access,
                rclose: self.rclose,
                perm: self.perm,
                kind: self.kind,
            })));
        }
        let mut components = path.components();
        let head = loop {
            let head = components.next();
            match head {
                Some(Component::Normal(x)) => break x,
                Some(Component::CurDir) => continue,
                Some(Component::RootDir) => continue,
                Some(Component::ParentDir) => Err(Error::NoSuchFileOrDirectory)?,
                None => "",
            };
        };
        if let Some(entry) = self
            .data
            .entries
            .read()
            .iter()
            .filter(|&x| x.name() == head)
            .next()
        {
            Ok(match entry {
                VDirEnt::File {
                    name: _,
                    perm,
                    data,
                } => Object::File(Box::new(VFile {
                    data: data.clone(),
                    access: None,
                    rclose: false,
                    perm: *perm,
                    kind: if data.append_only {
                        Kind::AppendOnly.into()
                    } else {
                        Kind::empty()
                    },
                })),
                VDirEnt::Dir {
                    name: _,
                    perm,
                    data,
                } => Object::Dir(Box::new(VDir {
                    data: data.clone(),
                    access: None,
                    rclose: false,
                    perm: *perm,
                    kind: Kind::Directory.into(),
                })),
            })
        } else {
            Err(Error::NoSuchFileOrDirectory)?
        }
    }

    async fn close(self) -> Result<()> {
        if self.rclose {
            return self.remove().await;
        }
        Ok(())
    }

    async fn remove(self) -> Result<()> {
        todo!()
    }

    async fn stat(&self) -> Result<Stat> {
        Ok(Stat {
            _type: 0,
            _dev: 0,
            version: 0,
            path: 0,
            mode: Mode {
                open: Flags {
                    access: self.access.ok_or(Error::BadFileDescriptor)?,
                    truncate: false,
                    rclose: self.rclose,
                },
                perm: self.perm,
                kind: self.kind,
            },
            length: self.data.entries.read().len(),
            ..Default::default()
        })
    }
}

#[async_trait]
impl FileLike for VFile {
    async fn open(&mut self, config: Flags) -> Result<()> {
        self.rclose = config.rclose;
        self.access = Some(config.access);
        if config.truncate {
            let mut data = self.data.bytes.write();
            data.clear();
        }
        Ok(())
    }

    async fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        if !self.access.ok_or(Error::BadFileDescriptor)?.read() {
            Err(Error::PermissionDenied)?;
        }
        let bytes = self.data.bytes.read();
        if offset >= bytes.len() {
            return Ok(0);
        }
        let n = core::cmp::min(buf.len(), bytes.len() - offset);
        buf[..n].copy_from_slice(&bytes[offset..offset + n]);
        Ok(n)
    }

    async fn write(&mut self, mut offset: usize, buf: &[u8]) -> Result<usize> {
        if !self.access.ok_or(Error::BadFileDescriptor)?.write() {
            Err(Error::PermissionDenied)?;
        }
        let mut bytes = self.data.bytes.write();
        if self.kind.contains(Kind::AppendOnly) {
            offset = bytes.len();
        }
        let end = offset + buf.len();
        if end > bytes.len() {
            bytes.resize(end, 0);
        }
        let n = buf.len();
        bytes[offset..offset + n].copy_from_slice(buf);
        Ok(n)
    }

    async fn close(self) -> Result<()> {
        if self.rclose {
            return self.remove().await;
        }
        Ok(())
    }

    async fn remove(self) -> Result<()> {
        todo!()
    }

    async fn stat(&self) -> Result<Stat> {
        Ok(Stat {
            mode: Mode {
                open: Flags {
                    access: self.access.ok_or(Error::BadFileDescriptor)?,
                    truncate: false,
                    rclose: self.rclose,
                },
                perm: self.perm,
                kind: self.kind,
            },
            length: self.data.bytes.read().len(),
            ..Default::default()
        })
    }

    async fn duplicate(&self) -> Result<File> {
        Ok(Box::new(self.clone()))
    }
}

impl From<VDirEnt> for DirEnt {
    fn from(value: VDirEnt) -> Self {
        let mut kind = Kind::empty();
        if value.append_only() {
            kind |= Kind::AppendOnly;
        }
        if value.dir() {
            kind |= Kind::Directory;
        }
        DirEnt {
            name: value.name().to_string(),
            perm: value.perm(),
            kind,
        }
    }
}
