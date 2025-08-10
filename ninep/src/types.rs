use super::*;
use alloc::fmt;
use chrono::{DateTime, Utc};
use enumflags2::bitflags;
use serde::{
    de::{self, SeqAccess, Visitor},
    ser::SerializeStruct,
};

#[derive(Copy, Clone, Eq, PartialEq, Serialize, Deserialize, Debug, Ord, PartialOrd, Hash)]
pub struct Fid(pub u32);

#[derive(Copy, Clone, Eq, PartialEq, Serialize, Deserialize, Debug, Ord, PartialOrd, Hash)]
pub struct Tag(pub u16);

#[derive(Copy, Clone, Eq, PartialEq, Serialize, Deserialize, Debug, Ord, PartialOrd, Hash, Default)]
pub struct Qid {
    pub flags: BitFlags<Flag>,
    pub version: u32,
    pub path: u64,
}

#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash, Serialize, Deserialize)]
pub enum Access {
    #[serde(rename = "0")]
    Read = 0,
    #[serde(rename = "1")]
    Write = 1,
    #[serde(rename = "2")]
    ReadWrite = 2,
    #[serde(rename = "3")]
    Execute = 3,
}

impl Access {
    pub fn read(&self) -> bool {
        *self == Access::Read || *self == Access::ReadWrite
    }

    pub fn write(&self) -> bool {
        *self == Access::Write || *self == Access::ReadWrite
    }
}

#[bitflags(default = OtherRead | GroupRead | OwnerWrite | OwnerRead)]
#[repr(u16)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum Perm {
    OtherExecute = 1 << 0,
    OtherWrite = 1 << 1,
    OtherRead = 1 << 2,
    GroupExecute = 1 << 3,
    GroupWrite = 1 << 4,
    GroupRead = 1 << 5,
    OwnerExecute = 1 << 6,
    OwnerWrite = 1 << 7,
    OwnerRead = 1 << 8,
}

impl Perm {
    pub fn all() -> BitFlags<Perm> {
        Perm::OtherExecute
            | Perm::OtherWrite
            | Perm::OtherRead
            | Perm::GroupExecute
            | Perm::GroupWrite
            | Perm::GroupRead
            | Perm::OwnerExecute
            | Perm::OwnerWrite
            | Perm::OwnerRead
    }

    pub fn default() -> BitFlags<Perm> {
        Default::default()
    }

    pub fn other() -> BitFlags<Perm> {
        Perm::OtherExecute | Perm::OtherWrite | Perm::OtherRead
    }

    pub fn group() -> BitFlags<Perm> {
        Perm::GroupExecute | Perm::GroupWrite | Perm::GroupRead
    }

    pub fn owner() -> BitFlags<Perm> {
        Perm::OwnerExecute | Perm::OwnerWrite | Perm::OwnerRead
    }
}

#[bitflags]
#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum Flag {
    Temporary = 1 << 2,
    Authentication = 1 << 3,
    Exclusive = 1 << 5,
    Append = 1 << 6,
    Directory = 1 << 7,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash, Default, Serialize, Deserialize)]
pub struct Mode {
    pub perm: BitFlags<Perm>,
    pub _skip: u8,
    pub flags: BitFlags<Flag>,
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
