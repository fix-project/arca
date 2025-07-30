use kernel::prelude::*;

use super::*;

pub struct Filesystem {
    data: FilesystemData,
    access: Option<Access>,
}

#[derive(Default, Clone)]
pub struct FilesystemData {
    mounts: Arc<RwLock<BTreeMap<PathBuf, MountEnt>>>,
}

#[derive(Clone)]
struct UnionData {
    entries: Arc<RwLock<Vec<Mount>>>,
}

struct Union {
    data: UnionData,
    access: Option<Access>,
}

struct Mount {
    object: Object,
    access: Option<Access>,
    create: bool,
}

impl Mount {
    pub fn new(object: Object, create: bool) -> Self {
        Mount {
            object,
            access: None,
            create,
        }
    }
}

enum MountEnt {
    Object(Object),
    Union(UnionData),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MountType {
    Replace,
    Before,
    After,
}

impl Filesystem {
    pub fn new(base: Dir) -> Self {
        let mut mounts = BTreeMap::new();
        mounts.insert(
            PathBuf::from("".to_owned()),
            MountEnt::Object(Object::Dir(base)),
        );
        let data = FilesystemData {
            mounts: Arc::new(RwLock::new(mounts)),
        };
        Filesystem {
            data: data,
            access: None,
        }
    }

    pub async fn bind(
        &mut self,
        name: &Path,
        old: &Path,
        mtype: MountType,
        create: bool,
    ) -> Result<()> {
        let name = name.relative();
        let repl = self.walk(name).await?;
        self.attach(repl, old, mtype, create).await
    }

    pub async fn attach(
        &mut self,
        object: Object,
        mtpt: &Path,
        mtype: MountType,
        create: bool,
    ) -> Result<()> {
        let mtpt = mtpt.relative();
        let orig = self.walk(mtpt).await?;

        let mut mounts = self.data.mounts.write();
        match mtype {
            MountType::Replace => {
                mounts.insert(mtpt.to_owned(), MountEnt::Object(object));
            }
            MountType::Before => {
                if orig.is_file() || object.is_file() {
                    return Err(Error::NotADirectory);
                }
                let mut orig = Some(orig);
                let mut repl = Some(object);
                mounts
                    .entry(mtpt.to_owned())
                    .and_modify(|x| {
                        common::util::replace_with(x, |x| match x {
                            MountEnt::Object(object) => MountEnt::Union(UnionData {
                                entries: Arc::new(RwLock::new(vec![
                                    Mount::new(repl.take().unwrap(), create),
                                    Mount::new(object, true),
                                ])),
                            }),
                            MountEnt::Union(union) => {
                                union
                                    .entries
                                    .write()
                                    .insert(0, Mount::new(repl.take().unwrap(), create));
                                MountEnt::Union(union)
                            }
                        });
                    })
                    .or_insert_with(|| {
                        MountEnt::Union(UnionData {
                            entries: Arc::new(RwLock::new(vec![
                                Mount::new(repl.take().unwrap(), create),
                                Mount::new(orig.take().unwrap(), create),
                            ])),
                        })
                    });
            }
            MountType::After => {
                if orig.is_file() || object.is_file() {
                    return Err(Error::NotADirectory);
                }
                let mut orig = Some(orig);
                let mut repl = Some(object);
                mounts
                    .entry(mtpt.to_owned())
                    .and_modify(|x| {
                        common::util::replace_with(x, |x| match x {
                            MountEnt::Object(object) => MountEnt::Union(UnionData {
                                entries: Arc::new(RwLock::new(vec![
                                    Mount::new(object, true),
                                    Mount::new(repl.take().unwrap(), create),
                                ])),
                            }),
                            MountEnt::Union(union) => {
                                union
                                    .entries
                                    .write()
                                    .push(Mount::new(repl.take().unwrap(), create));
                                MountEnt::Union(union)
                            }
                        });
                    })
                    .or_insert_with(|| {
                        MountEnt::Union(UnionData {
                            entries: Arc::new(RwLock::new(vec![
                                Mount::new(orig.take().unwrap(), create),
                                Mount::new(repl.take().unwrap(), create),
                            ])),
                        })
                    });
            }
        }
        Ok(())
    }

    pub async fn mount(
        &mut self,
        root: Dir,
        old: &Path,
        mtype: MountType,
        create: bool,
    ) -> Result<()> {
        self.attach(Object::Dir(root), old, mtype, create).await
    }
}

#[async_trait]
#[allow(unused_variables)]
impl DirLike for Filesystem {
    async fn open(&mut self, config: Flags) -> Result<()> {
        self.access = Some(config.access);
        Ok(())
    }

    async fn create(&mut self, name: &str, config: Mode) -> Result<Object> {
        let mut mounts = self.data.mounts.write();
        if mounts.contains_key(name) {
            return Err(Error::FileExists);
        }
        let mount = mounts.get_mut("").unwrap();
        match mount {
            MountEnt::Object(Object::Dir(d)) => d.create(name, config).await,
            MountEnt::Object(Object::File(_)) => return Err(Error::NotADirectory),
            MountEnt::Union(u) => {
                Union {
                    data: u.clone(),
                    access: self.access,
                }
                .create(name, config)
                .await
            }
        }
    }

    async fn read(&self, offset: usize, count: usize) -> Result<Vec<DirEnt>> {
        todo!();
    }

    async fn walk(&self, path: &Path) -> Result<Object> {
        if !self.access.ok_or(Error::BadFileDescriptor)?.read() {
            return Err(Error::PermissionDenied);
        }
        let path = path.relative();

        let mounts = self.data.mounts.read();
        let best = mounts.keys().filter(|m| path.starts_with(m)).fold(
            "".as_ref(),
            |a: &Path, x: &PathBuf| {
                if x.len() > a.len() { x } else { a }
            },
        );

        let rest = path.strip_prefix(best).unwrap();
        let mount = mounts.get(best).unwrap();
        match &mount {
            MountEnt::Object(object) => object.walk(rest).await,
            MountEnt::Union(union) => {
                Union {
                    data: union.clone(),
                    access: None,
                }
                .walk(rest)
                .await
            }
        }
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
}

#[async_trait]
#[allow(unused_variables)]
impl DirLike for Union {
    async fn open(&mut self, config: Flags) -> Result<()> {
        Ok(())
    }

    async fn create(&mut self, name: &str, config: Mode) -> Result<Object> {
        todo!()
    }

    async fn read(&self, offset: usize, count: usize) -> Result<Vec<DirEnt>> {
        todo!()
    }

    async fn walk(&self, path: &Path) -> Result<Object> {
        todo!()
    }

    async fn close(self) -> Result<()> {
        todo!()
    }

    async fn remove(self) -> Result<()> {
        todo!()
    }

    async fn stat(&self) -> Result<Stat> {
        todo!()
    }
}
