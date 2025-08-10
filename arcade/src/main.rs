#![no_main]
#![no_std]
#![feature(try_blocks)]
#![feature(try_trait_v2)]
#![feature(iterator_try_collect)]
#![allow(dead_code)]

use enumflags2::BitFlags;
use kernel::prelude::*;

mod vfs;
use crate::vfs::{
    dev::DevFS,
    fs::{Filesystem, MountType},
    mem::MemFS,
    vsock::VSockFS,
    *,
};
use ninep::*;

extern crate alloc;

const FOO: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/foo"));

#[kmain]
async fn main(_: &[usize]) {
    let x: Result<()> = try {
        let f: Function = common::elfloader::load_elf(FOO).unwrap();

        let root = VClosedDir::from(MemFS);
        let mut fs = Filesystem::new(root.into());

        fs.dup()
            .await?
            .create(
                "net",
                Perm::default(),
                Flag::Directory.into(),
                Access::ReadWrite,
            )
            .await?;
        let net: ClosedDir = fs.walk("net".as_ref()).await?.try_into().unwrap();

        net.create(
            "vsock",
            Perm::default(),
            Flag::Directory.into(),
            Access::ReadWrite,
        )
        .await?;
        fs.mount(
            VClosedDir::from(VSockFS).into(),
            "/net/vsock".as_ref(),
            MountType::Replace,
            true,
        )
        .await?;

        fs.create(
            "dev",
            Perm::default(),
            Flag::Directory.into(),
            Access::ReadWrite,
        )
        .await?;

        fs.mount(
            VClosedDir::from(DevFS).into(),
            "/dev".as_ref(),
            MountType::Replace,
            true,
        )
        .await?;

        let connect: ClosedDir = fs
            .walk("/net/vsock/connect".as_ref())
            .await?
            .try_into()
            .unwrap();

        fs.create(
            "mnt",
            Perm::default(),
            Flag::Directory.into(),
            Access::ReadWrite,
        )
        .await?;
        let conn: OpenFile = connect
            .create(
                "2:564",
                Perm::default(),
                Flag::Exclusive.into(),
                Access::ReadWrite,
            )
            .await?
            .try_into()
            .unwrap();
        let hostfs = Client::new(conn).await?.attach(None, "akshay", "").await?;

        fs.mount(hostfs.into(), "/mnt".as_ref(), MountType::Replace, true)
            .await?;

        let input = fs
            .create(
                "/mnt/input.txt",
                Perm::default(),
                BitFlags::default(),
                Access::Write,
            )
            .await?;
        let mut input: OpenFile = input.try_into().unwrap();
        input.write(b"hello from the kernel!").await?;
        let mut fs: ClosedDir = fs.into();

        let result = run(f, &mut fs).await?;

        log::info!("result: {result:?}");
        let output: ClosedFile = fs
            .walk(Path::new("/mnt/output.txt"))
            .await?
            .try_into()
            .unwrap();
        let mut output = output.open(Access::Read).await?;
        let mut buf = vec![0; 1024];
        let n = output.read(&mut buf).await?;
        buf.truncate(n);
        let output = String::from_utf8_lossy(&buf);
        log::info!("output: {output}");
    };
    x.unwrap();
}

async fn run(mut f: Function, fs: &mut ClosedDir) -> Result<Value> {
    let stdin = fs.walk("/dev/cons").await?.open(Access::ReadWrite).await?;
    let stdout = fs.walk("/dev/cons").await?.open(Access::ReadWrite).await?;
    let stderr = fs.walk("/dev/cons").await?.open(Access::ReadWrite).await?;

    let mut fds = Vec::new();
    fds.push(stdin);
    fds.push(stdout);
    fds.push(stderr);

    let cwd = "/";
    loop {
        let result = f.force();
        let Value::Function(g) = result else {
            break Ok(result);
        };
        if g.is_arcane() {
            f = g;
            continue;
        }
        let (effect, args) = g.into_inner().read();
        let effect: Blob = effect.try_into().unwrap();
        let args: Vec<Value> = args.into_iter().collect();
        f = match (&*effect, &*args) {
            (
                b"open",
                &[
                    Value::Blob(ref path),
                    Value::Word(flags),
                    Value::Word(mode),
                    Value::Function(ref k),
                ],
            ) => {
                let result: Result<Value> = try {
                    let mut path = PathBuf::from(String::from_utf8_lossy(path).into_owned());
                    let open = OpenFlags(flags.read() as u32);
                    let access: Access = open.try_into()?;
                    let flags = BitFlags::<Flag>::from(open);
                    let perm: BitFlags<Perm> = ModeT(mode.read() as u32).into();
                    if path.is_relative() {
                        path = PathBuf::from(cwd.to_owned()) + path.to_str();
                    }
                    let name = path.file_name().ok_or(Error::OperationNotPermitted)?;
                    let path = path.relative();
                    let parent = Path::new(path).parent().unwrap();
                    let dir: ClosedDir = fs.walk(parent).await?.try_into()?;
                    let file = dir.walk(name).await;
                    let file = match file {
                        Ok(file) => file.open(access).await?,
                        Err(e) => {
                            if !open.create() {
                                Err(e)?;
                            }
                            match dir
                                .dup()
                                .await?
                                .create(name, perm, flags, access)
                                .await
                            {
                                Ok(file) => file,
                                Err(_) => {
                                    let file = dir.walk(path.file_name().unwrap()).await?;
                                    file.open(access).await?
                                }
                            }
                        }
                    };
                    let index = fds.len();
                    fds.push(file);
                    Value::Word(Word::new(index as u64))
                };
                match result {
                    Ok(fd) => k.clone()(fd),
                    Err(e) => todo!("open: handle {e:?}"),
                }
            }
            (
                b"write",
                &[
                    Value::Word(fd),
                    Value::Blob(ref data),
                    Value::Function(ref k),
                ],
            ) => {
                let file: &mut OpenFile = (&mut fds[fd.read() as usize]).try_into().unwrap();
                let count = file.write(data).await;
                let count = count
                    .map(|x| Value::Word((x as u64).into())).expect("cannot handle bad syscall result");
                k.clone()(count)
            }
            (b"read", &[Value::Word(fd), Value::Word(count), Value::Function(ref k)]) => {
                let file: &mut OpenFile = (&mut fds[fd.read() as usize]).try_into().unwrap();
                let mut buf = vec![0; count.read() as usize];
                let count = file.read(&mut buf).await;
                let result = count
                    .map(|x| {
                        buf.truncate(x);
                        Value::Blob(Blob::from_inner(buf.into_boxed_slice().into()))
                    }).expect("cannot handle bad syscall result");
                k.clone()(result)
            }
            // (
            //     b"seek",
            //     &[
            //         Value::Word(fd),
            //         Value::Word(offset),
            //         Value::Word(_),
            //         Value::Function(ref k),
            //     ],
            // ) => k.clone()(0),
            (b"close", &[Value::Word(_), Value::Function(ref k)]) => k.clone()(0),
            (b"exit", [result, ..]) => {
                break Ok(result.clone());
            }
            _ => {
                panic!("invalid effect: {effect:?}({args:?})");
            }
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct OpenFlags(pub u32);

impl TryFrom<OpenFlags> for Access {
    type Error = Error;

    fn try_from(value: OpenFlags) -> Result<Self> {
        let value = value.0;
        let acc = value & arcane::O_ACCMODE;
        Ok(match acc {
            arcane::O_EXEC => Access::Execute,
            arcane::O_RDONLY => Access::Read,
            arcane::O_RDWR => Access::ReadWrite,
            arcane::O_WRONLY => Access::Write,
            #[allow(unreachable_patterns)]
            arcane::O_SEARCH => Access::Execute,
            _ => return Err(Error::OperationNotPermitted),
        })
    }
}

impl From<OpenFlags> for BitFlags<Flag> {
    fn from(value: OpenFlags) -> Self {
        let value = value.0;
        let mut b = BitFlags::default();
        if (value & arcane::O_DIRECTORY) != 0 {
            b |= Flag::Directory;
        }
        b
        // TODO: handle other flags (O_CREAT, O_APPEND, O_TRUNC)
    }
}

impl OpenFlags {
    pub fn create(&self) -> bool {
        (self.0 & arcane::O_CREAT) != 0
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct ModeT(pub u32);

impl From<ModeT> for BitFlags<Perm> {
    fn from(value: ModeT) -> Self {
        let value = value.0;
        let mut b = BitFlags::default();
        if (value & arcane::S_IXUSR) != 0 {
            b |= Perm::OwnerExecute;
        }
        if (value & arcane::S_IRUSR) != 0 {
            b |= Perm::OwnerRead;
        }
        if (value & arcane::S_IWUSR) != 0 {
            b |= Perm::OwnerWrite;
        }
        if (value & arcane::S_IXGRP) != 0 {
            b |= Perm::GroupExecute;
        }
        if (value & arcane::S_IRGRP) != 0 {
            b |= Perm::GroupRead;
        }
        if (value & arcane::S_IWGRP) != 0 {
            b |= Perm::GroupWrite;
        }
        if (value & arcane::S_IXOTH) != 0 {
            b |= Perm::OtherExecute;
        }
        if (value & arcane::S_IROTH) != 0 {
            b |= Perm::OtherRead;
        }
        if (value & arcane::S_IWOTH) != 0 {
            b |= Perm::OtherWrite;
        }
        b
    }
}
