use core::pin::Pin;

use alloc::collections::btree_map::BTreeMap;
use common::util::mutex::Mutex;
use either::Either;
use futures::StreamExt;

pub use super::*;

type SpawnFn<'a> = dyn Fn(Pin<Box<dyn Future<Output = ()> + Send>>) + 'a + Send + Sync;

pub struct Server<'a> {
    map: Arc<Mutex<BTreeMap<String, Box<dyn Dir>>>>,
    spawn: Box<SpawnFn<'a>>,
}

fn dir(fid: Fid) -> Qid {
    Qid {
        flags: Flag::Directory,
        version: 0,
        path: fid.0 as u64,
    }
}

fn file(fid: Fid) -> Qid {
    Qid {
        flags: Flags::empty(),
        version: 0,
        path: fid.0 as u64,
    }
}

fn obj(fid: Fid, object: &Object) -> Qid {
    if object.is_dir() { dir(fid) } else { file(fid) }
}

type ArcMutex<T> = Arc<Mutex<T>>;
type MaybeOpenFile = Either<(Box<dyn Dir>, String), Object>;

impl<'a> Server<'a> {
    pub fn new(
        spawn: impl Fn(Pin<Box<dyn Future<Output = ()> + Send>>) + 'a + Send + Sync,
    ) -> Self {
        Self {
            map: Default::default(),
            spawn: Box::new(spawn),
        }
    }

    pub async fn add(&mut self, name: &str, root: impl Dir) {
        self.map
            .lock()
            .await
            .insert(name.to_string(), Box::new(root));
    }

    pub async fn serve(&self, socket: impl File) -> Result<()> {
        let mut socket: Box<dyn File> = Box::new(socket);
        let fids: ArcMutex<BTreeMap<Fid, ArcMutex<MaybeOpenFile>>> = Default::default();
        loop {
            let mut size = [0; 4];
            socket.read(&mut size).await?;
            let size = u32::from_le_bytes(size) as usize;
            if size == 0 {
                return Ok(());
            }
            let mut request = vec![0; size - 4];
            socket.read(&mut request).await?;
            let request: TMessage = wire::from_bytes(&request)?;
            log::debug!("<- {request:?}");
            let mut socket = socket.dup().await?;
            let fids = fids.clone();
            let map = self.map.clone();
            let future = async move {
                let tag = request.tag();
                let f = async || {
                    match request {
                        TMessage::Version {
                            tag,
                            msize,
                            version,
                        } => {
                            if version.starts_with("9P2000") {
                                RMessage::Version {
                                    tag,
                                    msize,
                                    version: "9P2000".to_string(),
                                }
                            } else {
                                RMessage::Version {
                                    tag,
                                    msize,
                                    version: "unknown".to_string(),
                                }
                            }
                        }
                        TMessage::Auth { tag, .. } => RMessage::Error {
                            tag,
                            ename: "no auth required".to_string(),
                        },
                        TMessage::Attach {
                            tag, fid, aname, ..
                        } => {
                            let map = map.lock().await;
                            let root = map.get(&aname).ok_or(ErrorKind::NotFound)?.dup().await?;
                            core::mem::drop(map);
                            let qid = dir(fid);
                            fids.lock().await.insert(
                                fid,
                                Arc::new(Mutex::new(Either::Right(Object::Dir(root)))),
                            );
                            RMessage::Attach { tag, qid }
                        }
                        TMessage::Flush { .. } => todo!(),
                        TMessage::Walk {
                            tag,
                            fid,
                            newfid,
                            name,
                        } => {
                            let fids_ = fids.lock().await;
                            let object = fids_.get(&fid).ok_or(ErrorKind::NotFound)?.clone();
                            core::mem::drop(fids_);
                            let mut object = object.lock().await;
                            let path = PathBuf::from(name.join("/"));
                            let object = match &mut *object {
                                Either::Left((parent, name)) => {
                                    &parent.open(name, Open::Read).await?
                                }
                                Either::Right(obj) => obj,
                            };
                            let mut current: Object = object.dup().await?;
                            if path.is_empty() {
                                let qid = vec![obj(newfid, &current)];
                                fids.lock()
                                    .await
                                    .insert(newfid, Arc::new(Mutex::new(Either::Right(current))));
                                RMessage::Walk { tag, qid }
                            } else {
                                // TODO: this walk is messy, is there a simpler way to do it?
                                let mut qid = vec![];
                                let parent = if let Some(parent) = path.parent() {
                                    let components = parent.components();
                                    for component in components {
                                        if !current.is_dir() {
                                            break;
                                        }
                                        let cur_dir = current.as_dir().unwrap();
                                        let Ok(next) = cur_dir.walk(component, Open::Read).await
                                        else {
                                            current = Object::Dir(cur_dir);
                                            break;
                                        };
                                        qid.push(obj(Fid(!0), &next));
                                        current = next;
                                    }
                                    current
                                } else {
                                    current.dup().await?
                                };
                                let name = path.file_name().ok_or(ErrorKind::InvalidFilename)?;
                                let parent = parent.as_dir()?;
                                let mut isdir: bool = false;
                                let dirents: Vec<Result<DirEnt>> =
                                    parent.readdir().await?.collect().await;
                                for x in dirents {
                                    let x = x?;
                                    isdir |= x.dir;
                                }
                                if isdir {
                                    qid.push(dir(Fid(!0)));
                                } else {
                                    qid.push(file(Fid(!0)));
                                }
                                fids.lock().await.insert(
                                    newfid,
                                    Arc::new(Mutex::new(Either::Left((parent, name.to_string())))),
                                );
                                RMessage::Walk { tag, qid }
                            }
                        }
                        TMessage::Open { tag, fid, access } => {
                            let fids = fids.lock().await;
                            let f = fids.get(&fid).ok_or(ErrorKind::InvalidInput)?.clone();
                            core::mem::drop(fids);
                            let mut node = f.lock().await;
                            let (parent, name) =
                                node.as_mut().left().ok_or(ErrorKind::InvalidInput)?;
                            let object = parent.open(name, access.try_into()?).await?;
                            let qid = obj(fid, &object);
                            *node = Either::Right(object);
                            RMessage::Open {
                                tag,
                                qid,
                                iounit: 0,
                            }
                        }
                        TMessage::Create {
                            tag,
                            fid,
                            name,
                            mode,
                            access,
                        } => {
                            let node = fids
                                .lock()
                                .await
                                .remove(&fid)
                                .ok_or(ErrorKind::NotFound)?
                                .clone();
                            let new = match &mut *node.lock().await {
                                Either::Left((parent, name)) => {
                                    let mut parent = parent.open(name, Open::ReadWrite).await?;
                                    parent
                                        .as_dir_mut()?
                                        .dyn_create(name, mode.try_into()?, access.try_into()?)
                                        .await?
                                }
                                Either::Right(obj) => {
                                    obj.as_dir_mut()?
                                        .dyn_create(&name, mode.try_into()?, access.try_into()?)
                                        .await?
                                }
                            };
                            let qid = obj(fid, &new);
                            fids.lock()
                                .await
                                .insert(fid, Arc::new(Mutex::new(Either::Right(new))));
                            RMessage::Create {
                                tag,
                                qid,
                                iounit: 0,
                            }
                        }
                        TMessage::Read {
                            tag,
                            fid,
                            offset,
                            count,
                        } => {
                            let fids = fids.lock().await;
                            let f = fids.get(&fid).ok_or(ErrorKind::InvalidInput)?.clone();
                            core::mem::drop(fids);
                            let mut f = f.lock().await;
                            let o = f.as_mut().right().ok_or(ErrorKind::InvalidInput)?;
                            let data = match o {
                                Object::File(f) => {
                                    if f.seekable() {
                                        f.seek(SeekFrom::Start(offset as usize)).await?;
                                    }
                                    let mut data = vec![0; count as usize];
                                    let count = f.read(&mut data).await?;
                                    data.truncate(count);
                                    data
                                }
                                Object::Dir(d) => {
                                    let mut bytes = vec![];
                                    let v: Vec<Result<Stat>> = d
                                        .readdir()
                                        .await?
                                        .skip(offset as usize)
                                        .take(count as usize)
                                        .map(|x| {
                                            x.map(|x| Stat {
                                                qid: if x.dir {
                                                    dir(Fid(!0))
                                                } else {
                                                    file(Fid(!0))
                                                },
                                                name: x.name.clone(),
                                                ..Default::default()
                                            })
                                        })
                                        .collect()
                                        .await;
                                    for x in v {
                                        let x = x?;
                                        let y: &[u8] = unsafe {
                                            core::slice::from_raw_parts(
                                                &x as *const Stat as *const u8,
                                                core::mem::size_of::<Stat>(),
                                            )
                                        };
                                        bytes.extend(y);
                                    }
                                    bytes
                                }
                            };
                            RMessage::Read { tag, data }
                        }
                        TMessage::Write {
                            tag,
                            fid,
                            offset,
                            data,
                        } => {
                            let fids = fids.lock().await;
                            let f = fids.get(&fid).ok_or(ErrorKind::InvalidInput)?.clone();
                            core::mem::drop(fids);
                            let mut f = f.lock().await;
                            let f = f
                                .as_mut()
                                .right()
                                .ok_or(ErrorKind::InvalidInput)?
                                .as_file_mut()?;
                            if f.seekable() {
                                f.dyn_seek(SeekFrom::Start(offset as usize)).await?;
                            }
                            let count = f.dyn_write(&data).await? as u32;
                            RMessage::Write { tag, count }
                        }
                        TMessage::Clunk { tag, fid } => {
                            fids.lock().await.remove(&fid);
                            RMessage::Clunk(tag)
                        }
                        TMessage::Remove { .. } => todo!("remove"),
                        TMessage::Stat { .. } => todo!("stat"),
                        TMessage::WStat { .. } => todo!("wstat"),
                        _ => {
                            Err(ErrorKind::Unsupported)?;
                            unreachable!()
                        }
                    }
                };
                let mut response = f().await;
                response.set_tag(tag);
                log::debug!("-> {response:?}");
                let response = wire::to_bytes_with_len(response).unwrap();
                socket.write(&response).await.unwrap();
            };
            (self.spawn)(Box::pin(future));
        }
    }
}
