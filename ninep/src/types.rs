use super::*;
use ::bitflags::bitflags;
use alloc::fmt;
use chrono::{DateTime, Utc};
use serde::{
    de::{self, SeqAccess, Visitor},
    ser::SerializeStruct,
};

#[derive(Copy, Clone, Eq, PartialEq, Serialize, Deserialize, Debug, Ord, PartialOrd, Hash)]
pub struct Fid(pub u32);

#[derive(Copy, Clone, Eq, PartialEq, Serialize, Deserialize, Debug, Ord, PartialOrd, Hash)]
pub struct Tag(pub u16);

#[derive(
    Copy, Clone, Eq, PartialEq, Serialize, Deserialize, Debug, Ord, PartialOrd, Hash, Default,
)]
pub struct Qid {
    pub flags: Flags,
    pub version: u32,
    pub path: u64,
}

bitflags! {
    #[derive(Copy, Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
    pub struct Access: u8 {
        const Read = 0;
        const Write = 1;
        const ReadWrite = 2;
        const Execute = 3;
        const Truncate = 0x10;
        const RemoveOnClose = 0x40;
        const _ = !0;
    }
}

impl Access {
    pub fn read(&self) -> bool {
        let access = self.bits() & 0xf;
        access == 0x0 || access == 0x2
    }

    pub fn write(&self) -> bool {
        let access = self.bits() & 0xf;
        access == 0x1 || access == 0x2
    }

    pub fn execute(&self) -> bool {
        let access = self.bits() & 0xf;
        access == 0x3
    }

    pub fn truncate(&self) -> bool {
        self.contains(Access::Truncate)
    }

    pub fn rclose(&self) -> bool {
        self.contains(Access::RemoveOnClose)
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Default, Hash, Ord, PartialOrd)]
    pub struct Perm: u16 {
        const OtherExecute = 1 << 0;
        const OtherWrite = 1 << 1;
        const OtherRead = 1 << 2;
        const GroupExecute = 1 << 3;
        const GroupWrite = 1 << 4;
        const GroupRead = 1 << 5;
        const OwnerExecute = 1 << 6;
        const OwnerWrite = 1 << 7;
        const OwnerRead = 1 << 8;
    }
}

pub type Perms = Perm;

impl Perm {
    pub fn dir() -> Perm {
        Perm::all() & !(Perm::GroupWrite | Perm::GroupExecute)
    }

    pub fn other() -> Perm {
        Perm::OtherExecute | Perm::OtherWrite | Perm::OtherRead
    }

    pub fn group() -> Perm {
        Perm::GroupExecute | Perm::GroupWrite | Perm::GroupRead
    }

    pub fn owner() -> Perm {
        Perm::OwnerExecute | Perm::OwnerWrite | Perm::OwnerRead
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Default, Hash, Ord, PartialOrd)]
    pub struct Flag: u8 {
        const Temporary = 1 << 2;
        const Authentication = 1 << 3;
        const Exclusive = 1 << 5;
        const Append = 1 << 6;
        const Directory = 1 << 7;
    }
}

pub type Flags = Flag;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash, Default, Serialize, Deserialize)]
pub struct Mode {
    pub perm: Perms,
    pub _skip: u8,
    pub flags: Flags,
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Serialize, Deserialize, Default)]
pub struct Stat {
    pub mtype: u16,
    pub dev: u32,
    pub qid: Qid,
    pub mode: Mode,
    pub atime: DateTime<Utc>,
    pub mtime: DateTime<Utc>,
    pub length: u64,
    pub name: String,
    pub uid: String,
    pub gid: String,
    pub muid: String,
}

#[derive(Clone, Eq, PartialEq, Debug, Hash)]
pub struct WireStat(pub Stat);

impl Serialize for WireStat {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let len = core::mem::size_of::<u16>()
            + core::mem::size_of::<u32>()
            + core::mem::size_of::<Qid>()
            + core::mem::size_of::<Mode>()
            + core::mem::size_of::<u32>()
            + core::mem::size_of::<u32>()
            + core::mem::size_of::<u64>()
            + (core::mem::size_of::<u16>() + self.0.name.len())
            + (core::mem::size_of::<u16>() + self.0.uid.len())
            + (core::mem::size_of::<u16>() + self.0.gid.len())
            + (core::mem::size_of::<u16>() + self.0.muid.len());
        let mut s = serializer.serialize_struct("WireStat", 2)?;
        s.serialize_field("len", &(len as u16))?;
        s.serialize_field("stat", &self.0)?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for WireStat {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct StatVisitor;

        impl<'de> Visitor<'de> for StatVisitor {
            type Value = WireStat;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Duration")
            }

            fn visit_seq<V>(self, mut seq: V) -> core::result::Result<WireStat, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let _: u16 = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let stat = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                Ok(WireStat(stat))
            }
        }

        deserializer.deserialize_struct("WireStat", &["len", "stat"], StatVisitor)
    }
}

impl From<vfs::Open> for Access {
    fn from(value: vfs::Open) -> Self {
        if value.contains(vfs::Open::ReadWrite) {
            Access::ReadWrite
        } else if value.contains(vfs::Open::Write) {
            Access::Write
        } else {
            Access::Read
        }
    }
}

impl TryFrom<Access> for vfs::Open {
    type Error = Error;

    fn try_from(value: Access) -> Result<Self> {
        let mut out = vfs::Open::empty();
        if value.read() {
            out |= vfs::Open::Read;
        }
        if value.write() {
            out |= vfs::Open::Write;
        }
        if value.truncate() {
            out |= vfs::Open::Truncate;
        }
        if value.rclose() {
            return Err(ErrorKind::Unsupported.into());
        }
        Ok(out)
    }
}

impl From<vfs::Create> for Mode {
    fn from(value: vfs::Create) -> Self {
        let perm = Perm::from_bits_truncate(value.perm() as u16);
        let flags = if value.dir() {
            Flag::Directory
        } else {
            Flag::empty()
        };
        Mode {
            perm,
            _skip: 0,
            flags,
        }
    }
}

impl TryFrom<Mode> for vfs::Create {
    type Error = Error;

    fn try_from(value: Mode) -> Result<Self> {
        let Mode {
            perm,
            _skip: _,
            flags,
        } = value;
        let mut value = vfs::Create::from_bits_truncate(perm.bits() as u64);
        if flags == Flag::Directory {
            value |= vfs::Create::Directory;
        } else if flags != Flag::empty() {
            return Err(ErrorKind::Unsupported.into());
        }
        Ok(value)
    }
}
