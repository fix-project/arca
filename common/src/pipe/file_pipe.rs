#![allow(unused)]
extern crate alloc;
use alloc::boxed::Box;

use crate::impl_tag_enum;
use crate::message::frame_codec::Frame;
use crate::message::traits::{
    ensure_empty, extract_field, write_field, FieldTagRepresentation, Tag, TagRepresentation,
    VariableMsg,
};
use crate::message::traits::{Error, IntoBytes};
use crate::pipe::pipe_type::{PipeProtocol, PipeType};
use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FileOpenFlags: u8 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const CREATE = 1 << 2;
        const APPEND = 1 << 3;
        const TRUNCATE = 1 << 4;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePipeHeader {
    pub flags: FileOpenFlags,
    pub path: Box<[u8]>,
}

impl VariableMsg for FilePipeHeader {
    fn encoded_len(&self) -> usize {
        size_of::<FileOpenFlags>() + FieldTagRepresentation::TAG_SIZE + self.path.len()
    }

    fn encode(&self, out: &mut [u8]) -> Result<(), Error> {
        if out.len() != self.encoded_len() {
            return Err(Error::SerializerError);
        }
        out[0] = self.flags.bits();
        write_field(&mut out[size_of::<FileOpenFlags>()..], &self.path)?;
        Ok(())
    }

    fn decode(input: &[u8]) -> Result<Self, Error> {
        let flags = FileOpenFlags::from_bits_truncate(*input.first().ok_or(Error::ParserError)?);
        let (path, rest) = extract_field(&input[size_of::<FileOpenFlags>()..])?;
        ensure_empty(rest)?;
        Ok(Self {
            flags,
            path: path.into(),
        })
    }
}

pub struct FilePipe;

impl PipeProtocol for FilePipe {
    const TYPE: PipeType = PipeType::File;
    type Header = FilePipeHeader;
}

impl_tag_enum! {
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Whence {
        Start = 0,
        Current = 1,
        End = 2,
    }
}

impl_tag_enum! {
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum FileRequestType {
        Read = 0,
        Write = 1,
        Seek = 2,
        Close = 3,
    }
}

impl_tag_enum! {
    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum FileReplyType {
        Read = 0,
        Wrote = 1,
        Seeked = 2,
        Closed = 3,
        Error = 4,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileRequest {
    Read { read_length: usize },
    Write { data: Box<[u8]> },
    Seek { offset: i64, whence: Whence },
    Close,
}

impl FileRequest {
    fn request_type(&self) -> FileRequestType {
        match self {
            FileRequest::Read { .. } => FileRequestType::Read,
            FileRequest::Write { .. } => FileRequestType::Write,
            FileRequest::Seek { .. } => FileRequestType::Seek,
            FileRequest::Close => FileRequestType::Close,
        }
    }
}

impl VariableMsg for FileRequest {
    fn encoded_len(&self) -> usize {
        size_of::<FileRequestType>()
            + match self {
                FileRequest::Read { .. } => usize::TAG_SIZE,
                FileRequest::Write { data } => data.len(),
                FileRequest::Seek { .. } => i64::TAG_SIZE + Whence::SIZE,
                FileRequest::Close => 0,
            }
    }

    fn encode(&self, out: &mut [u8]) -> Result<(), Error> {
        if out.len() != self.encoded_len() {
            return Err(Error::SerializerError);
        }
        // First byte is request type
        self.request_type().write_tag(out)?;
        let body = &mut out[FileRequestType::SIZE..];
        match self {
            FileRequest::Read { read_length } => {
                read_length.write_le(body)?;
            }
            FileRequest::Write { data } => {
                body.copy_from_slice(data);
            }
            FileRequest::Seek { offset, whence } => {
                offset.write_le(body)?;
                whence.write_tag(&mut body[i64::TAG_SIZE..])?;
            }
            FileRequest::Close => {}
        }
        Ok(())
    }

    fn decode(input: &[u8]) -> Result<Self, Error> {
        let (request_type_tag, body) = FileRequestType::extract_tag(input)?;
        match request_type_tag {
            FileRequestType::Read => {
                let (read_length, rest) = usize::extract_le(body)?;
                ensure_empty(rest)?;
                Ok(FileRequest::Read { read_length })
            }
            FileRequestType::Write => Ok(FileRequest::Write { data: body.into() }),
            FileRequestType::Seek => {
                let (offset, rest) = i64::extract_le(body)?;
                let (whence, rest) = Whence::extract_tag(rest)?;
                ensure_empty(rest)?;
                Ok(FileRequest::Seek { offset, whence })
            }
            FileRequestType::Close => {
                ensure_empty(body)?;
                Ok(FileRequest::Close)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileReply {
    Read { data: Box<[u8]> },
    Wrote { length: usize },
    Seeked { position: usize },
    Closed,
    Error { errno: i32 },
}

impl FileReply {
    fn reply_type(&self) -> FileReplyType {
        match self {
            FileReply::Read { .. } => FileReplyType::Read,
            FileReply::Wrote { .. } => FileReplyType::Wrote,
            FileReply::Seeked { .. } => FileReplyType::Seeked,
            FileReply::Closed => FileReplyType::Closed,
            FileReply::Error { .. } => FileReplyType::Error,
        }
    }
}

impl VariableMsg for FileReply {
    fn encoded_len(&self) -> usize {
        FileReplyType::SIZE
            + match self {
                FileReply::Read { data } => data.len(),
                FileReply::Wrote { .. } => usize::TAG_SIZE,
                FileReply::Seeked { .. } => usize::TAG_SIZE,
                FileReply::Closed => 0,
                FileReply::Error { .. } => i32::TAG_SIZE,
            }
    }

    fn encode(&self, out: &mut [u8]) -> Result<(), Error> {
        if out.len() != self.encoded_len() {
            return Err(Error::SerializerError);
        }
        self.reply_type().write_tag(out)?;
        let body = &mut out[FileReplyType::SIZE..];
        match self {
            FileReply::Read { data } => {
                body.copy_from_slice(data);
            }
            FileReply::Wrote { length } => {
                length.write_le(body)?;
            }
            FileReply::Seeked { position } => {
                position.write_le(body)?;
            }
            FileReply::Closed => {}
            FileReply::Error { errno } => {
                errno.write_le(body)?;
            }
        }
        Ok(())
    }

    fn decode(input: &[u8]) -> Result<Self, Error> {
        let (reply_type_tag, body) = FileReplyType::extract_tag(input)?;
        match reply_type_tag {
            FileReplyType::Read => Ok(FileReply::Read { data: body.into() }),
            FileReplyType::Wrote => {
                let (length, rest) = usize::extract_le(body)?;
                ensure_empty(rest)?;
                Ok(FileReply::Wrote { length })
            }
            FileReplyType::Seeked => {
                let (position, rest) = usize::extract_le(body)?;
                ensure_empty(rest)?;
                Ok(FileReply::Seeked { position })
            }
            FileReplyType::Closed => {
                ensure_empty(body)?;
                Ok(FileReply::Closed)
            }
            FileReplyType::Error => {
                let (errno, rest) = i32::extract_le(body)?;
                ensure_empty(rest)?;
                Ok(FileReply::Error { errno })
            }
        }
    }
}

impl IntoBytes for FileRequest {
    fn into_boxed_slice(self) -> Box<[u8]> {
        self.to_boxed_slice().unwrap()
    }
}

impl IntoBytes for FileReply {
    fn into_boxed_slice(self) -> Box<[u8]> {
        self.to_boxed_slice().unwrap()
    }
}

impl<const MAX: usize> TryFrom<&Frame<MAX>> for FileRequest {
    type Error = Error;
    fn try_from(f: &Frame<MAX>) -> Result<Self, Error> {
        FileRequest::decode(f.as_slice())
    }
}
impl<const MAX: usize> TryFrom<Frame<MAX>> for FileRequest {
    type Error = Error;
    fn try_from(f: Frame<MAX>) -> Result<Self, Error> {
        FileRequest::decode(f.as_slice())
    }
}
impl<const MAX: usize> TryFrom<&Frame<MAX>> for FileReply {
    type Error = Error;
    fn try_from(f: &Frame<MAX>) -> Result<Self, Error> {
        FileReply::decode(f.as_slice())
    }
}
impl<const MAX: usize> TryFrom<Frame<MAX>> for FileReply {
    type Error = Error;
    fn try_from(f: Frame<MAX>) -> Result<Self, Error> {
        FileReply::decode(f.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipe::pipe_type::encode_initialization;

    fn check_request(request: FileRequest) {
        let bytes = request.to_boxed_slice().unwrap();
        assert_eq!(bytes.len(), request.encoded_len());
        assert_eq!(FileRequest::decode(&bytes).unwrap(), request);
    }
    fn check_reply(reply: FileReply) {
        let bytes = reply.to_boxed_slice().unwrap();
        assert_eq!(bytes.len(), reply.encoded_len());
        assert_eq!(FileReply::decode(&bytes).unwrap(), reply);
    }

    #[test]
    fn flag_round_trip() {
        let flags = FileOpenFlags::READ | FileOpenFlags::WRITE | FileOpenFlags::APPEND;
        assert_eq!(FileOpenFlags::from_bits_truncate(flags.bits()), flags);
    }

    #[test]
    fn header_round_trip() {
        let header = FilePipeHeader {
            flags: FileOpenFlags::READ | FileOpenFlags::CREATE,
            path: Box::from(&b"/dir/test/test.txt"[..]),
        };
        let frame_payload = encode_initialization::<FilePipe>(&header).unwrap();
        let (kind, body) = PipeType::extract_tag(&frame_payload).unwrap();
        assert_eq!(kind, PipeType::File);
        assert_eq!(FilePipeHeader::decode(body).unwrap(), header);
    }

    #[test]
    fn valid_empty_path() {
        let header = FilePipeHeader {
            flags: FileOpenFlags::empty(),
            path: Box::from(&b""[..]),
        };
        let bytes = header.to_boxed_slice().unwrap();
        assert_eq!(FilePipeHeader::decode(&bytes).unwrap(), header);
    }

    #[test]
    fn requests_round_trip() {
        check_request(FileRequest::Read { read_length: 4096 });
        check_request(FileRequest::Write {
            data: Box::from(&b"hello world"[..]),
        });
        check_request(FileRequest::Write {
            data: Box::from(&b""[..]),
        });
        check_request(FileRequest::Seek {
            offset: -42,
            whence: Whence::End,
        });
        check_request(FileRequest::Close);
    }

    #[test]
    fn replies_round_trip() {
        check_reply(FileReply::Read {
            data: Box::from(&b"data"[..]),
        });
        check_reply(FileReply::Read {
            data: Box::from(&b""[..]),
        });
        check_reply(FileReply::Wrote { length: 11 });
        check_reply(FileReply::Seeked { position: 1024 });
        check_reply(FileReply::Closed);
        check_reply(FileReply::Error { errno: -2 });
    }

    #[test]
    fn unknown_request_tag_rejected() {
        assert_eq!(FileRequest::decode(&[0xFF]), Err(Error::ParserError));
    }

    #[test]
    fn frame_tryfrom_matches_decode() {
        let request = FileRequest::Seek {
            offset: 7,
            whence: Whence::Start,
        };
        let bytes = request.to_boxed_slice().unwrap();
        let mut frame = Frame::<64> {
            payload_len: bytes.len(),
            payload: [0u8; 64],
        };
        frame.payload[..bytes.len()].copy_from_slice(&bytes);
        assert_eq!(FileRequest::try_from(&frame).unwrap(), request);
    }
}
