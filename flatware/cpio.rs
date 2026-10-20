#![no_std]
extern crate alloc;

use alloc::{collections::BTreeMap, format, rc::Rc, vec::Vec};
use core::ops::Range;
use fix::files::{self, Stat};
use fix::{Any, Blob, Cast, DataType, Error, Handle, Object, Tree, Value, tree};

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

type Expression = Rc<dyn Value<Type = Any>>;
type Inode = (u32, u32, u32);

#[derive(Clone, Default)]
struct Data {
    archive: Rc<Vec<u8>>,
    range: Range<usize>,
}

impl AsRef<[u8]> for Data {
    fn as_ref(&self) -> &[u8] {
        &self.archive[self.range.clone()]
    }
}

struct Entry {
    stat: Stat,
    children: BTreeMap<Vec<u8>, Entry>,
    data: Data,
    inode: Option<Inode>,
    explicit: bool,
}

impl Entry {
    fn directory() -> Self {
        Self {
            stat: Stat {
                filetype: 3,
                nlink: 1,
                ..Stat::default()
            },
            children: BTreeMap::new(),
            data: Data::default(),
            inode: None,
            explicit: false,
        }
    }

    fn insert(&mut self, path: &[&[u8]], entry: Entry) {
        assert_eq!(self.stat.filetype, 3, "CPIO parent is not a directory");
        if let Some((name, rest)) = path.split_first() {
            self.children
                .entry(name.to_vec())
                .or_insert_with(Self::directory)
                .insert(rest, entry);
        } else {
            assert!(!self.explicit, "duplicate CPIO entry");
            assert!(
                self.children.is_empty() || entry.stat.filetype == 3,
                "CPIO parent is not a directory"
            );
            self.stat = entry.stat;
            self.data = entry.data;
            self.inode = entry.inode;
            self.explicit = true;
        }
    }

    fn descriptor(mut self, name: Vec<u8>, links: &BTreeMap<Inode, Data>) -> Expression {
        let contents: Expression = if self.stat.filetype == 3 {
            self.stat.size = self.children.len() as u64;
            let children = self
                .children
                .into_iter()
                .map(|(name, child)| child.descriptor(name, links))
                .collect::<Vec<_>>();
            Rc::new(Tree::create(children).erase())
        } else {
            let data = self
                .inode
                .and_then(|inode| links.get(&inode).cloned())
                .unwrap_or(self.data);
            self.stat.size = data.as_ref().len() as u64;
            Rc::new(Blob::create(data).erase())
        };
        Rc::new(tree![Blob::create(name), files::write_stat(&self.stat), contents].erase())
    }
}

fn take<'a>(source: &mut &'a [u8], length: usize) -> Result<&'a [u8], Error> {
    let (bytes, rest) = source.split_at_checked(length).ok_or(Error::OutOfBounds)?;
    *source = rest;
    Ok(bytes)
}

fn expand(source: &dyn Value<Type = Object<Blob>>) -> Result<Expression, Error> {
    let bytes = Rc::new(source.read()?);
    let mut archive = &bytes[..];
    let mut root = Entry::directory();
    let mut links = BTreeMap::new();
    while !archive.is_empty() {
        let header = take(&mut archive, 110)?;
        assert!(
            matches!(&header[..6], b"070701" | b"070702"),
            "expected newc or crc CPIO"
        );
        let mut fields = [0u32; 13];
        for (field, bytes) in fields.iter_mut().zip(header[6..].as_chunks::<8>().0) {
            *field = u32::from_str_radix(
                core::str::from_utf8(bytes).expect("invalid CPIO header"),
                16,
            )
            .expect("invalid CPIO field");
        }
        let name = take(&mut archive, fields[11] as usize)?;
        let end = name
            .iter()
            .position(|byte| *byte == 0)
            .expect("unterminated CPIO name");
        assert!(
            name[end..].iter().all(|byte| *byte == 0),
            "invalid CPIO name"
        );
        let name = &name[..end];
        take(
            &mut archive,
            (110usize + fields[11] as usize).wrapping_neg() & 3,
        )?;
        let start = bytes.len() - archive.len();
        let data = take(&mut archive, fields[6] as usize)?;
        take(&mut archive, data.len().wrapping_neg() & 3)?;
        if &header[..6] == b"070702" && fields[1] & 0xf000 == 0x8000 {
            assert_eq!(
                fields[12],
                data.iter()
                    .fold(0u32, |sum, byte| sum.wrapping_add(u32::from(*byte))),
                "CPIO checksum mismatch"
            );
        }
        if name == b"TRAILER!!!" {
            assert!(data.is_empty(), "CPIO trailer has data");
            assert!(
                archive.iter().all(|byte| *byte == 0),
                "data after CPIO trailer"
            );
            break;
        }
        assert!(
            !name.is_empty() && !name.starts_with(b"/"),
            "invalid CPIO path"
        );
        let path = name
            .split(|byte| *byte == b'/')
            .filter(|part| !part.is_empty() && *part != b".")
            .collect::<Vec<_>>();
        assert!(!path.contains(&&b".."[..]), "invalid CPIO path");
        let filetype = match fields[1] & 0xf000 {
            0x4000 => 3,
            0x8000 => 4,
            0xa000 => 7,
            _ => panic!("unsupported CPIO file type"),
        };
        assert!(filetype != 3 || data.is_empty(), "CPIO directory has data");
        assert!(filetype != 7 || !data.contains(&0), "NUL in symlink target");
        let inode = (fields[7], fields[8], fields[0]);
        let linked = filetype != 3 && fields[4] > 1;
        let data = Data {
            archive: bytes.clone(),
            range: start..start + data.len(),
        };
        if linked
            && !data.as_ref().is_empty()
            && let Some(previous) = links.insert(inode, data.clone())
        {
            assert_eq!(
                previous.as_ref(),
                data.as_ref(),
                "conflicting CPIO hard-link data"
            );
        }
        root.insert(
            &path,
            Entry {
                stat: Stat {
                    filetype,
                    nlink: u64::from(fields[4]),
                    size: data.as_ref().len() as u64,
                    mtim: u64::from(fields[5]) * 1_000_000_000,
                    ..Stat::default()
                },
                children: BTreeMap::new(),
                data,
                inode: linked.then_some(inode),
                explicit: true,
            },
        );
    }
    Ok(root.descriptor(Vec::new(), &links))
}

fn record(output: &mut Vec<u8>, path: &[u8], stat: &Stat, size: usize, inode: u32) {
    let mode = match stat.filetype {
        3 => 0x41ed,
        4 => 0x81a4,
        7 => 0xa1ff,
        _ => panic!("unsupported Flatware file type"),
    };
    let fields = [
        inode,
        mode,
        0,
        0,
        u32::try_from(stat.nlink).expect("link count exceeds CPIO range"),
        u32::try_from(stat.mtim / 1_000_000_000).expect("mtime exceeds CPIO range"),
        u32::try_from(size).expect("file exceeds CPIO range"),
        0,
        0,
        0,
        0,
        u32::try_from(path.len() + 1).expect("name exceeds CPIO range"),
        0,
    ];
    output.extend_from_slice(b"070701");
    for field in fields {
        output.extend_from_slice(format!("{field:08x}").as_bytes());
    }
    output.extend_from_slice(path);
    output.push(0);
    output.resize((output.len() + 3) & !3, 0);
}

fn archive_len(
    descriptor: &dyn Value<Type = Object<Tree>>,
    path_len: usize,
) -> Result<usize, Error> {
    let metadata: Cast<Object<Tree>, _> = files::stat(&descriptor)?.try_into()?;
    let mut length = (110 + path_len + 1 + 3) & !3;
    if files::read_stat(&metadata)?.filetype == 3 {
        let children: Cast<Object<Tree>, _> = files::contents(&descriptor)?.try_into()?;
        for index in 0..usize::try_from(children.len()?)? {
            let child: Cast<Object<Tree>, _> = children.get(index).try_into()?;
            length = length
                .checked_add(archive_len(
                    &child,
                    path_len + 1 + usize::try_from(files::name(&child)?.len()?)?,
                )?)
                .ok_or(Error::OutOfBounds)?;
        }
    } else {
        let contents: Cast<Object<Blob>, _> = files::contents(&descriptor)?.try_into()?;
        length = length
            .checked_add((usize::try_from(contents.len()?)? + 3) & !3)
            .ok_or(Error::OutOfBounds)?;
    }
    Ok(length)
}

fn pack(
    descriptor: &dyn Value<Type = Object<Tree>>,
    path: &[u8],
    output: &mut Vec<u8>,
    inode: &mut u32,
) -> Result<(), Error> {
    let metadata: Cast<Object<Tree>, _> = files::stat(&descriptor)?.try_into()?;
    let stat = files::read_stat(&metadata)?;
    assert_eq!(stat.dev, 0, "unsupported Flatware device");
    *inode = inode.checked_add(1).expect("too many CPIO entries");
    if stat.filetype == 3 {
        record(output, path, &stat, 0, *inode);
        let children: Cast<Object<Tree>, _> = files::contents(&descriptor)?.try_into()?;
        let mut names = alloc::collections::BTreeSet::new();
        for index in 0..usize::try_from(children.len()?)? {
            let child: Cast<Object<Tree>, _> = children.get(index).try_into()?;
            let name = files::name(&child)?.read()?;
            assert!(
                !name.is_empty()
                    && name != b"."
                    && name != b".."
                    && !name.contains(&b'/')
                    && !name.contains(&0),
                "invalid directory entry name"
            );
            assert!(names.insert(name.clone()), "duplicate directory entry");
            let mut child_path = path.to_vec();
            child_path.push(b'/');
            child_path.extend_from_slice(&name);
            pack(&child, &child_path, output, inode)?;
        }
    } else {
        let contents: Cast<Object<Blob>, _> = files::contents(&descriptor)?.try_into()?;
        let length = usize::try_from(contents.len()?)?;
        record(output, path, &stat, length, *inode);
        let start = output.len();
        output.resize(start + length, 0);
        let data = &mut output[start..];
        contents.read_into(0, data)?;
        assert!(
            stat.filetype != 7 || !data.contains(&0),
            "NUL in symlink target"
        );
        output.resize((output.len() + 3) & !3, 0);
    }
    Ok(())
}

#[fix::apply]
pub fn apply(combination: &Handle<Object<Tree>, fix::Focus>) -> Result<Expression, Error> {
    assert_eq!(
        combination.len()?,
        3,
        "expected a CPIO Blob or Flatware directory"
    );
    let input = combination.get(2);
    if input.data_type()? == DataType::Blob {
        expand(&Handle::<Object<Blob>, _>::try_from(input)?)
    } else {
        let directory = Handle::<Object<Tree>, _>::try_from(input)?;
        let metadata: Cast<Object<Tree>, _> = files::stat(&directory)?.try_into()?;
        assert_eq!(
            files::read_stat(&metadata)?.filetype,
            3,
            "expected a Flatware directory"
        );
        let length = archive_len(&directory, 1)?
            .checked_add(124 + 511)
            .ok_or(Error::OutOfBounds)?
            & !511;
        let mut output = Vec::with_capacity(length);
        pack(&directory, b".", &mut output, &mut 0)?;
        record(
            &mut output,
            b"TRAILER!!!",
            &Stat {
                filetype: 4,
                ..Stat::default()
            },
            0,
            0,
        );
        output.resize((output.len() + 511) & !511, 0);
        Ok(Rc::new(Blob::create(output).erase()))
    }
}
