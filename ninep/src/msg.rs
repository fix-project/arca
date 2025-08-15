use super::*;
use alloc::format;
use core::{
    convert::Infallible,
    ops::{ControlFlow, FromResidual, Try},
};

#[derive(Debug, Clone)]
pub struct Error(String);

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "9P: {}", self.0)
    }
}
impl core::error::Error for Error {}

impl From<Error> for vfs::Error {
    fn from(value: Error) -> Self {
        vfs::Error::other(value)
    }
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub enum TMessage {
    #[serde(rename = "100")]
    Version {
        tag: Tag,
        msize: u32,
        version: String,
    },
    #[serde(rename = "102")]
    Auth {
        tag: Tag,
        afid: Fid,
        uname: String,
        aname: String,
    },
    #[serde(rename = "104")]
    Attach {
        tag: Tag,
        fid: Fid,
        afid: Fid,
        uname: String,
        aname: String,
        n_uname: Option<u32>,
    },
    // 106 would be TError, but it doesn't exist
    #[serde(rename = "108")]
    Flush { tag: Tag, oldtag: Tag },
    #[serde(rename = "110")]
    Walk {
        tag: Tag,
        fid: Fid,
        newfid: Fid,
        name: Vec<String>,
    },
    #[serde(rename = "112")]
    Open { tag: Tag, fid: Fid, access: Access },
    #[serde(rename = "114")]
    Create {
        tag: Tag,
        fid: Fid,
        name: String,
        mode: Mode,
        access: Access,
    },
    #[serde(rename = "116")]
    Read {
        tag: Tag,
        fid: Fid,
        offset: u64,
        count: u32,
    },
    #[serde(rename = "118")]
    Write {
        tag: Tag,
        fid: Fid,
        offset: u64,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    #[serde(rename = "120")]
    Clunk { tag: Tag, fid: Fid },
    #[serde(rename = "122")]
    Remove { tag: Tag, fid: Fid },
    #[serde(rename = "124")]
    Stat { tag: Tag, fid: Fid },
    #[serde(rename = "126")]
    WStat { tag: Tag, fid: Fid, stat: Stat },
    #[serde(rename = "12")]
    LOpen { tag: Tag, fid: Fid, flags: u32 },
    #[serde(rename = "14")]
    LCreate {
        tag: Tag,
        fid: Fid,
        name: String,
        flags: u32,
        mode: u32,
        gid: u32,
    },
    #[serde(rename = "72")]
    LMkdir {
        tag: Tag,
        fid: Fid,
        name: String,
        mode: u32,
        gid: u32,
    },
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub enum RMessage {
    #[serde(rename = "101")]
    Version {
        tag: Tag,
        msize: u32,
        version: String,
    },
    #[serde(rename = "103")]
    Auth { tag: Tag, aqid: Qid },
    #[serde(rename = "105")]
    Attach { tag: Tag, qid: Qid },
    #[serde(rename = "107")]
    Error { tag: Tag, ename: String },
    #[serde(rename = "109")]
    Flush { tag: Tag },
    #[serde(rename = "111")]
    Walk { tag: Tag, qid: Vec<Qid> },
    #[serde(rename = "113")]
    Open { tag: Tag, qid: Qid, iounit: u32 },
    #[serde(rename = "115")]
    Create { tag: Tag, qid: Qid, iounit: u32 },
    #[serde(rename = "117")]
    Read {
        tag: Tag,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    },
    #[serde(rename = "119")]
    Write { tag: Tag, count: u32 },
    #[serde(rename = "121")]
    Clunk(Tag),
    #[serde(rename = "123")]
    Remove(Tag),
    #[serde(rename = "125")]
    Stat { tag: Tag, stat: Stat },
    #[serde(rename = "127")]
    WStat { tag: Tag },
    #[serde(rename = "7")]
    LError { tag: Tag, ecode: u32 },
    #[serde(rename = "13")]
    LOpen { tag: Tag, qid: Qid, iounit: u32 },
    #[serde(rename = "15")]
    LCreate { tag: Tag, qid: Qid, iounit: u32 },
    #[serde(rename = "73")]
    LMkdir { tag: Tag, qid: Qid },
}

impl TMessage {
    pub fn tag(&self) -> Tag {
        *match self {
            TMessage::Version { tag, .. } => tag,
            TMessage::Auth { tag, .. } => tag,
            TMessage::Attach { tag, .. } => tag,
            TMessage::Flush { tag, .. } => tag,
            TMessage::Walk { tag, .. } => tag,
            TMessage::Open { tag, .. } => tag,
            TMessage::Create { tag, .. } => tag,
            TMessage::Read { tag, .. } => tag,
            TMessage::Write { tag, .. } => tag,
            TMessage::Clunk { tag, .. } => tag,
            TMessage::Remove { tag, .. } => tag,
            TMessage::Stat { tag, .. } => tag,
            TMessage::WStat { tag, .. } => tag,
            TMessage::LOpen { tag, .. } => tag,
            TMessage::LCreate { tag, .. } => tag,
            TMessage::LMkdir { tag, .. } => tag,
        }
    }
}

impl RMessage {
    pub fn tag(&self) -> Tag {
        *match self {
            RMessage::Version { tag, .. } => tag,
            RMessage::Auth { tag, .. } => tag,
            RMessage::Attach { tag, .. } => tag,
            RMessage::Error { tag, .. } => tag,
            RMessage::Flush { tag } => tag,
            RMessage::Walk { tag, .. } => tag,
            RMessage::Open { tag, .. } => tag,
            RMessage::Create { tag, .. } => tag,
            RMessage::Read { tag, .. } => tag,
            RMessage::Write { tag, .. } => tag,
            RMessage::Clunk(tag) => tag,
            RMessage::Remove(tag) => tag,
            RMessage::Stat { tag, .. } => tag,
            RMessage::WStat { tag, .. } => tag,
            RMessage::LError { tag, .. } => tag,
            RMessage::LOpen { tag, .. } => tag,
            RMessage::LCreate { tag, .. } => tag,
            RMessage::LMkdir { tag, .. } => tag,
        }
    }

    pub fn set_tag(&mut self, newtag: Tag) {
        match self {
            RMessage::Version { tag, .. } => *tag = newtag,
            RMessage::Auth { tag, .. } => *tag = newtag,
            RMessage::Attach { tag, .. } => *tag = newtag,
            RMessage::Error { tag, .. } => *tag = newtag,
            RMessage::Flush { tag } => *tag = newtag,
            RMessage::Walk { tag, .. } => *tag = newtag,
            RMessage::Open { tag, .. } => *tag = newtag,
            RMessage::Create { tag, .. } => *tag = newtag,
            RMessage::Read { tag, .. } => *tag = newtag,
            RMessage::Write { tag, .. } => *tag = newtag,
            RMessage::Clunk(tag) => *tag = newtag,
            RMessage::Remove(tag) => *tag = newtag,
            RMessage::Stat { tag, .. } => *tag = newtag,
            RMessage::WStat { tag, .. } => *tag = newtag,
            RMessage::LError { tag, .. } => *tag = newtag,
            RMessage::LOpen { tag, .. } => *tag = newtag,
            RMessage::LCreate { tag, .. } => *tag = newtag,
            RMessage::LMkdir { tag, .. } => *tag = newtag,
        }
    }
}

impl Try for RMessage {
    type Output = RMessage;

    type Residual = core::result::Result<Infallible, Error>;

    fn from_output(output: Self::Output) -> Self {
        output
    }

    fn branch(self) -> core::ops::ControlFlow<Self::Residual, Self::Output> {
        match self {
            RMessage::Error { tag: _, ename } => ControlFlow::Break(Err(Error(ename))),
            RMessage::LError { tag: _, ecode } => {
                ControlFlow::Break(Err(Error(alloc::format!("Linux Error {ecode}"))))
            }
            x => ControlFlow::Continue(x),
        }
    }
}

impl FromResidual<core::result::Result<Infallible, derive_more::TryIntoError<vfs::Object>>>
    for RMessage
{
    fn from_residual(
        residual: core::result::Result<Infallible, derive_more::TryIntoError<vfs::Object>>,
    ) -> Self {
        let e: vfs::ErrorKind = residual.unwrap_err().into();
        Self::Error {
            tag: Tag(!0),
            ename: format!("VFS error: {e:?}"),
        }
    }
}

impl FromResidual<core::result::Result<Infallible, vfs::ErrorKind>> for RMessage {
    fn from_residual(residual: core::result::Result<Infallible, vfs::ErrorKind>) -> Self {
        Self::Error {
            tag: Tag(!0),
            ename: format!("VFS error: {residual:?}"),
        }
    }
}

impl FromResidual<core::result::Result<Infallible, vfs::Error>> for RMessage {
    fn from_residual(residual: core::result::Result<Infallible, vfs::Error>) -> Self {
        Self::Error {
            tag: Tag(!0),
            ename: format!("VFS error: {residual:?}"),
        }
    }
}

impl FromResidual<core::result::Result<Infallible, Error>> for RMessage {
    fn from_residual(residual: <Self as Try>::Residual) -> Self {
        Self::Error {
            tag: Tag(!0),
            ename: format!("9P error: {residual:?}"),
        }
    }
}

impl FromResidual<core::result::Result<Infallible, RMessage>> for RMessage {
    fn from_residual(residual: core::result::Result<Infallible, RMessage>) -> Self {
        residual.unwrap_err()
    }
}
