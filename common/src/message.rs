use crate::sendable::Sendable;

extern crate alloc;

#[derive(Debug)]
#[repr(C)]
pub enum Request {
    Nop,
    CreateNull,
    CreateWord { value: u64 },
    CreateAtom { ptr: usize, len: usize },
    CreateBlob { ptr: usize, len: usize },
    CreateTree { ptr: usize, len: usize },
    CreateThunk { src: usize },
    Read { src: usize },
    Apply { src: usize, arg: usize },
    Run { src: usize },
    Clone { src: usize },
    Drop { src: usize },
}

unsafe impl Sendable for Request {}

#[derive(Debug)]
#[repr(C)]
pub enum Type {
    Null,
    Word,
    Atom,
    Blob,
    Tree,
    Lambda,
    Thunk,
}

unsafe impl Sendable for Type {}

#[derive(Debug)]
#[repr(C)]
pub enum Response {
    Ack,
    Null,
    Word(u64),
    Atom { ptr: usize, len: usize },
    Blob { ptr: usize, len: usize },
    Tree { ptr: usize, len: usize },
    Type(Type),
    Handle(usize),
}

unsafe impl Sendable for Response {}

#[derive(Debug)]
#[repr(C)]
pub struct MetaRequest {
    pub seqno: usize,
    pub body: Request,
}

unsafe impl Sendable for MetaRequest {}

#[derive(Debug)]
#[repr(C)]
pub struct MetaResponse {
    pub seqno: usize,
    pub body: Response,
}

unsafe impl Sendable for MetaResponse {}
