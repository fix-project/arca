use crate::sendable::Sendable;

extern crate alloc;

#[derive(Debug)]
#[repr(C)]
pub enum Request {
    Nop,
    Resize { size: usize },
    Null { dst: usize },
    CreateWord { dst: usize, value: u64 },
    CreateAtom { dst: usize, ptr: usize, len: usize },
    CreateBlob { dst: usize, ptr: usize, len: usize },
    CreateTree { dst: usize, ptr: usize, len: usize },
    CreateThunk { dst: usize, src: usize },
    Read { src: usize },
    Apply { dst: usize, src: usize },
    Run { dst: usize, src: usize },
    Clone { dst: usize, src: usize },
    Drop { dst: usize },
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
    Null,
    Word(u64),
    Atom { ptr: usize, len: usize },
    Blob { ptr: usize, len: usize },
    Tree { ptr: usize, len: usize },
    Type(Type),
    Handle { index: usize },
}

unsafe impl Sendable for Response {}

#[derive(Debug)]
#[repr(C)]
pub enum MetaRequest {
    OpenPort { port: usize, size: usize },
    Request { port: usize, body: Request },
    ClosePort { port: usize },
    Exit,
}

unsafe impl Sendable for MetaRequest {}

#[derive(Debug)]
#[repr(C)]
pub enum MetaResponse {
    Response { port: usize, body: Response },
    Exit,
}

unsafe impl Sendable for MetaResponse {}
