use kernel::prelude::*;
use ninep::*;

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

#[derive(Clone)]
struct Union {
    data: UnionData,
    access: Option<Access>,
}

struct Mount {
    object: ClosedNode,
    access: Option<Access>,
    create: bool,
}

impl Mount {
    pub fn new(object: ClosedNode, create: bool) -> Self {
        Mount {
            object,
            access: None,
            create,
        }
    }
}

enum MountEnt {
    Node(ClosedNode),
    Union(UnionData),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MountType {
    Replace,
    Before,
    After,
}

impl Filesystem {
    pub fn new(base: ClosedDir) -> Self {
        let mut mounts = BTreeMap::new();
        mounts.insert(
            PathBuf::from("".to_owned()),
            MountEnt::Node(ClosedNode::Dir(base)),
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
        object: ClosedNode,
        mtpt: &Path,
        mtype: MountType,
        create: bool,
    ) -> Result<()> {
        let mtpt = mtpt.relative();
        let orig = self.walk(mtpt).await?;

        let mut mounts = self.data.mounts.write();
        match mtype {
            MountType::Replace => {
                mounts.insert(mtpt.to_owned(), MountEnt::Node(object));
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
                            MountEnt::Node(object) => MountEnt::Union(UnionData {
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
                            MountEnt::Node(object) => MountEnt::Union(UnionData {
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
        root: ClosedDir,
        old: &Path,
        mtype: MountType,
        create: bool,
    ) -> Result<()> {
        self.attach(ClosedNode::Dir(root), old, mtype, create).await
    }
}

#[async_trait]
impl NodeLike for Filesystem {
    async fn stat(&self) -> Result<Stat> {
        todo!()
    }

    async fn wstat(&mut self, _: &Stat) -> Result<()> {
        todo!()
    }

    async fn clunk(self: Box<Self>) -> Result<()> {
        todo!()
    }

    async fn remove(self: Box<Self>) -> Result<()> {
        todo!()
    }

    fn qid(&self) -> Qid {
        todo!()
    }
}

#[async_trait]
impl ClosedNodeLike for Filesystem {}

#[async_trait]
impl OpenNodeLike for Filesystem {}

#[async_trait]
impl DirLike for Filesystem {}

#[async_trait]
impl ClosedDirLike for Filesystem {
    async fn open(mut self: Box<Self>, access: Access) -> Result<OpenDir> {
        self.access = Some(access);
        Ok((*self).into())
    }

    async fn walk(&self, path: &Path) -> Result<ClosedNode> {
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
            MountEnt::Node(object) => object.walk(rest).await,
            MountEnt::Union(_) => {
                todo!();
            }
        }
    }

    async fn create(
        self: Box<Self>,
        name: &str,
        perm: BitFlags<Perm>,
        flags: BitFlags<Flag>,
        access: Access,
    ) -> Result<OpenNode> {
        Filesystem::create(&self, name, perm, flags, access).await
    }
}

impl Filesystem {
    pub async fn create(
        &self,
        name: &str,
        perm: BitFlags<Perm>,
        flags: BitFlags<Flag>,
        access: Access,
    ) -> Result<OpenNode> {
        let mut mounts = self.data.mounts.write();
        if mounts.contains_key(name) {
            return Err(Error::FileExists);
        }
        let mount = mounts.get_mut("").unwrap();
        match mount {
            MountEnt::Node(ClosedNode::Dir(d)) => {
                d.dup().await?.create(name, perm, flags, access).await
            }
            MountEnt::Node(ClosedNode::File(_)) => return Err(Error::NotADirectory),
            MountEnt::Union(_) => {
                todo!();
            }
        }
    }
}

#[async_trait]
impl OpenDirLike for Filesystem {
    async fn read(&self, _offset: usize, _count: usize) -> Result<Vec<Stat>> {
        todo!();
    }
}
