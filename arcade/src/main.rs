#![no_main]
#![no_std]
#![feature(try_blocks)]
#![allow(dead_code)]

use kernel::prelude::*;

use crate::vfs::*;

// mod ninep;
mod vfs;
// mod vsock;

extern crate alloc;

const FOO: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/foo"));

#[kmain]
async fn main(_: &[usize]) {
    let x: Result<()> = try {
        let f: Function = common::elfloader::load_elf(FOO).unwrap();

        let mut root: Dir = Box::new(VDir::new());
        root.open(Flags {
            access: Access::ReadWrite,
            ..Default::default()
        })
        .await?;

        let mut fs = Filesystem::new(root);
        fs.open(Flags::default()).await.unwrap();

        fs.create(
            "dev",
            Mode {
                kind: Kind::Directory.into(),
                ..Default::default()
            },
        )
        .await?;
        let mut devfs: Dir = Box::new(DevFS);
        devfs.open(Flags::default()).await?;
        fs.mount(devfs, "dev".as_ref(), MountType::Replace, true)
            .await?;

        let input = fs
            .create(
                "input.txt",
                Mode {
                    open: Flags {
                        access: Access::Write,
                        truncate: true,
                        rclose: false,
                    },
                    perm: Perms::default(),
                    kind: Kind::AppendOnly.into(),
                },
            )
            .await?;
        let mut input: File = input.try_into().unwrap();
        input.write(0, b"hello from the kernel!").await?;
        let mut fs: Dir = Box::new(fs);

        let result = run(f, &mut fs).await?;

        log::info!("result: {result:?}");
        let mut output: File = fs.walk(Path::new("/output.txt")).await?.try_into().unwrap();
        output
            .open(Flags {
                access: Access::Read,
                truncate: false,
                rclose: false,
            })
            .await?;
        let mut buf = vec![0; 1024];
        let n = output.read(0, &mut buf).await?;
        buf.truncate(n);
        let output = String::from_utf8_lossy(&buf);
        log::info!("output: {output}");
    };
    x.unwrap();
}

async fn run(mut f: Function, fs: &mut Dir) -> Result<Value> {
    let mut stdin = fs.walk("/dev/null".as_ref()).await?;
    stdin.open(Flags::default()).await?;
    let mut stdout = fs.walk("/dev/cons".as_ref()).await?;
    stdout.open(Flags::default()).await?;
    let mut stderr = fs.walk("/dev/cons".as_ref()).await?;
    stderr.open(Flags::default()).await?;

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
                    Value::Blob(ref name),
                    Value::Word(_),
                    Value::Word(_),
                    Value::Function(ref k),
                ],
            ) => {
                let name = String::from_utf8_lossy(name);
                let path = cwd.to_owned() + &name;
                let result = fs.create(&path, Mode::default()).await;
                let response = match result {
                    Ok(file) => {
                        let index = fds.len();
                        fds.push(file);
                        Value::Word(Word::new(index as u64))
                    }
                    Err(e) => todo!("handle: {e:?}"),
                };
                k.clone()(response)
            }
            (
                b"write",
                &[
                    Value::Word(fd),
                    Value::Blob(ref data),
                    Value::Function(ref k),
                ],
            ) => {
                let file: &mut File = (&mut fds[fd.read() as usize]).try_into().unwrap();
                let count = file.write(0, data).await;
                let (Ok(count) | Err(count)) = count
                    .map(|x| Value::Word((x as u64).into()))
                    .map_err(|_| Value::Exception(Exception::new(Blob::from("error"))));
                k.clone()(count)
            }
            (b"read", &[Value::Word(fd), Value::Word(count), Value::Function(ref k)]) => {
                let file: &File = (&fds[fd.read() as usize]).try_into().unwrap();
                let mut buf = vec![0; count.read() as usize];
                let count = file.read(0, &mut buf).await;
                let (Ok(result) | Err(result)) = count
                    .map(|x| {
                        buf.truncate(x);
                        Value::Blob(Blob::from_inner(buf.into_boxed_slice().into()))
                    })
                    .map_err(|_| Value::Exception(Exception::new(Blob::from("error"))));
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
