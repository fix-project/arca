use crate::sendable::Sendable;

extern crate alloc;

#[derive(Debug)]
#[repr(C)]
pub struct Handle {
    pub parts: [usize; 3],
    pub datatype: Type,
}

impl Handle {
    /// # Safety
    ///
    /// The copied handle does not affect the reference count within the kernel, and therefore must
    /// only be provided to RPC calls which leak the provided handle.
    pub unsafe fn copy(&self) -> Handle {
        Handle {
            parts: self.parts,
            datatype: self.datatype
        }
    }

    pub fn null() -> Handle {
        Handle {
            parts: [0; 3],
            datatype: Type::Null,
        }
    }

    pub fn is_null(&self) -> bool {
        self.datatype == Type::Null
    }

    pub fn word(value: u64) -> Handle {
        Handle {
            parts: [value as usize, 0, 0],
            datatype: Type::Word,
        }
    }

    pub fn get_word(&self) -> Option<u64> {
        if self.datatype == Type::Word {
            Some(self.parts[0] as u64)
        } else {
            None
        }
    }

    /// # Safety
    /// The pointer and length must be reconstructible as a Blob.
    pub unsafe fn blob(ptr: usize, len: usize) -> Handle {
        Handle {
            parts: [ptr, len, 0],
            datatype: Type::Blob,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub enum Type {
    Null,
    Error,
    Word,
    Atom,
    Blob,
    Tree,
    Page,
    PageTable,
    Lambda,
    Thunk,
}

#[derive(Debug)]
#[repr(C)]
pub enum Request {
    Nop,
    CreateBlob { ptr: usize, len: usize },
    CreateTree { ptr: usize, len: usize },
    ReadBlob(Handle),
    ReadTree(Handle),
    LoadElf(Handle),
    Apply(Handle, Handle),
    Run(Handle),
    Clone(Handle),
    Drop(Handle),
}

unsafe impl Sendable for Request {}

#[derive(Debug)]
#[repr(C)]
pub enum Response {
    Ack,
    Handle(Handle),
    Span {ptr: usize, len: usize},
}

unsafe impl Sendable for Response {}

#[derive(Debug)]
#[repr(C)]
pub struct MetaRequest {
    pub function: usize,
    pub context: usize,
    pub body: Request,
}

unsafe impl Sendable for MetaRequest {}

#[derive(Debug)]
#[repr(C)]
pub struct MetaResponse {
    pub function: usize,
    pub context: usize,
    pub body: Response,
}

unsafe impl Sendable for MetaResponse {}
