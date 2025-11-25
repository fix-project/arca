use alloc::collections::btree_map::BTreeMap;
use kernel::prelude::*;
use ninep::*;
use vfs::*;

// use super::*;

#[derive(Clone)]
pub struct Namespace {
    data: NamespaceData,
}

#[derive(Default, Clone)]
pub struct NamespaceData {
    mounts: Arc<RwLock<BTreeMap<PathBuf, MountEnt>>>,
}

#[derive(Clone)]
struct UnionData {
    entries: Arc<RwLock<Vec<Mount>>>,
}

#[derive(Clone)]
struct Union {
    data: UnionData,
}

struct Mount {
    dir: Box<dyn Dir>,
    create: bool,
}

impl Mount {
    pub fn new(dir: impl Dir, create: bool) -> Self {
        Mount {
            dir: dir.boxed(),
            create,
        }
    }
}

enum MountEnt {
    File(Box<dyn Dir>),
    Union(UnionData),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MountType {
    Replace,
    Before,
    After,
}

impl Namespace {
    pub fn new(base: impl Dir) -> Self {
        let mut mounts = BTreeMap::new();
        let base: Box<dyn Dir> = base.boxed();
        mounts.insert(PathBuf::from("".to_owned()), MountEnt::File(base));
        let data = NamespaceData {
            mounts: Arc::new(RwLock::new(mounts)),
        };
        Namespace { data }
    }

    // pub async fn bind(
    //     &mut self,
    //     name: &Path,
    //     old: &Path,
    //     mtype: MountType,
    //     create: bool,
    // ) -> Result<()> {
    //     todo!();
    //     // let name = name.relative();
    //     // let repl = self.walk(name).await?;
    //     // self.attach(repl, old, mtype, create).await
    // }

    pub async fn attach(
        &mut self,
        dir: impl Dir,
        mtpt: impl AsRef<Path>,
        mtype: MountType,
        create: bool,
    ) -> Result<()> {
        log::info!(
            "Namespace::attach: mtpt={:?}, mtype={:?}, create={}",
            mtpt.as_ref(),
            mtype,
            create
        );
        let mtpt = mtpt.as_ref().relative();
        let orig = self.walk(mtpt, Open::Read).await?.as_dir()?;

        let mut mounts = self.data.mounts.write();
        match mtype {
            MountType::Replace => {
                mounts.insert(mtpt.to_owned(), MountEnt::File(dir.boxed()));
            }
            MountType::Before => {
                let mut orig = Some(orig);
                let mut repl = Some(dir);
                mounts
                    .entry(mtpt.to_owned())
                    .and_modify(|x| {
                        common::util::replace_with(x, |x| match x {
                            MountEnt::File(object) => MountEnt::Union(UnionData {
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
                let mut orig = Some(orig);
                let mut repl = Some(dir);
                mounts
                    .entry(mtpt.to_owned())
                    .and_modify(|x| {
                        common::util::replace_with(x, |x| match x {
                            MountEnt::File(object) => MountEnt::Union(UnionData {
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

    // pub async fn mount(
    //     &mut self,
    //     root: ClosedDir,
    //     old: &Path,
    //     mtype: MountType,
    //     create: bool,
    // ) -> Result<()> {
    //     self.attach(ClosedNode::Dir(root), old, mtype, create).await
    // }

    pub async fn walk(&self, path: impl AsRef<Path>, open: Open) -> Result<Object> {
        let path = path.as_ref().to_owned();
        let path = path.relative();
        log::info!("Namespace::walk: path={:?}, open={:?}", path, open);

        let mounts = self.data.mounts.read();
        log::info!("Namespace::walk: mounts={:?}", mounts.keys());
        let best = mounts.keys().filter(|m| path.starts_with(m)).fold(
            "".as_ref(),
            |a: &Path, x: &PathBuf| {
                if x.len() > a.len() { x } else { a }
            },
        );
        let rest = path.strip_prefix(best).unwrap();
        let mount = mounts.get(best).unwrap();
        log::info!("Namespace::walk: best={:?}, rest={:?}", best, rest);
        match &mount {
            MountEnt::File(object) => object.walk(rest, open).await,
            MountEnt::Union(u) => {
                let u = u.entries.read();
                for mount in u.iter() {
                    if let Ok(x) = mount.dir.walk(rest, open).await {
                        return Ok(x);
                    }
                }
                Err(ErrorKind::NotFound.into())
            }
        }
    }

    pub async fn mkdir(&self, name: impl AsRef<Path>) -> Result<Object> {
        self.create(
            name,
            Create::Directory | Create::from_bits_truncate(0o755),
            Open::ReadWrite,
        )
        .await
    }

    pub async fn create(
        &self,
        path: impl AsRef<Path>,
        create: Create,
        open: Open,
    ) -> Result<Object> {
        let path = path.as_ref().to_owned();
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
        match mount {
            MountEnt::File(d) => {
                log::info!("Namespace::create: rest={:?}", rest);
                let parent = rest.parent();
                let name = rest.file_name().ok_or(ErrorKind::NotFound)?;
                let parent = if let Some(parent) = parent
                    && !parent.is_empty()
                {
                    d.walk(parent, Open::ReadWrite).await?.as_dir()?
                } else {
                    d.dup().await?
                };
                parent.create(name, create, open).await
            }
            MountEnt::Union(_) => {
                todo!("create in union mount");
            }
        }
    }

    pub async fn create_or_open(
        &self,
        path: impl AsRef<Path>,
        create: Create,
        open: Open,
    ) -> Result<Object> {
        let path = path.as_ref().to_owned();
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
        match mount {
            MountEnt::File(d) => {
                let parent = rest.parent();
                let name = rest.file_name().ok_or(ErrorKind::NotFound)?;
                let parent = if let Some(parent) = parent
                    && !parent.is_empty()
                {
                    d.walk(parent, Open::ReadWrite).await?.as_dir()?
                } else {
                    d.dup().await?
                };
                parent.create_or_open(name, create, open).await
            }
            MountEnt::Union(_) => {
                todo!("create in union mount");
            }
        }
    }
}
