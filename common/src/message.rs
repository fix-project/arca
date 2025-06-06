use core::ptr::Pointee;

use arca::DataType;

use crate::{sendable::Sendable, BuddyAllocator};

extern crate alloc;
use alloc::boxed::Box;

#[derive(Debug)]
#[repr(C)]
pub struct Handle {
    pub parts: [usize; 2],
    pub datatype: DataType,
}

impl Handle {
    /// # Safety
    ///
    /// The copied handle does not affect the reference count within the kernel, and therefore must
    /// only be provided to RPC calls which leak the provided handle.
    pub unsafe fn copy(&self) -> Handle {
        Handle {
            parts: self.parts,
            datatype: self.datatype,
        }
    }

    pub fn new(datatype: DataType, data: (usize, usize)) -> Self {
        Handle {
            parts: [data.0, data.1],
            datatype,
        }
    }

    pub fn read(&self) -> (usize, usize) {
        (self.parts[0], self.parts[1])
    }

    pub fn null() -> Handle {
        Handle {
            parts: [0; 2],
            datatype: DataType::Null,
        }
    }

    pub fn is_null(&self) -> bool {
        self.datatype == DataType::Null
    }

    pub fn word(value: u64) -> Handle {
        Handle {
            parts: [value as usize, 0],
            datatype: DataType::Word,
        }
    }

    pub fn get_word(&self) -> Option<u64> {
        if self.datatype == DataType::Word {
            Some(self.parts[0] as u64)
        } else {
            None
        }
    }

    /// # Safety
    /// The pointer and length must be reconstructible as a Blob.
    pub unsafe fn blob(ptr: usize, len: usize) -> Handle {
        Handle {
            parts: [ptr, len],
            datatype: DataType::Blob,
        }
    }

    pub fn from_value<T: arca::ValueType + Pointee<Metadata = ()>>(value: T) -> Handle {
        let value = Box::new_in(value, BuddyAllocator);
        let raw = Box::into_raw(value);
        Handle {
            parts: [raw as usize, 0],
            datatype: T::DATATYPE,
        }
    }

    pub fn to_value<T: arca::ValueType + Pointee<Metadata = ()>>(handle: Handle) -> T {
        let raw = handle.parts[0] as *mut _;
        let value = unsafe {
            assert_eq!(handle.datatype(), T::DATATYPE);
            Box::from_raw_in(raw, BuddyAllocator)
        };
        *value
    }

    pub fn datatype(&self) -> DataType {
        self.datatype
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
pub struct PageTableEntry {
    pub unique: bool,
    pub handle: Option<Handle>,
}

#[derive(Debug)]
#[repr(C)]
pub enum Request {
    Nop,
    Clone(Handle),
    Drop(Handle),
    Type(Handle),
    CreateError(Handle),
    CreateAtom {
        ptr: usize,
        len: usize,
    },
    CreateBlob {
        ptr: usize,
        len: usize,
    },
    CreateTree {
        size: usize,
    },
    CreatePage {
        size: usize,
    },
    CreateTable {
        size: usize,
    },
    CreateLambda {
        thunk: Handle,
        index: usize,
    },
    CreateThunk {
        registers: Handle,
        memory: Handle,
        descriptors: Handle,
    },
    ReadBlob(Handle),
    ReadPage(Handle),
    WritePage {
        handle: Handle,
        offset: usize,
        ptr: usize,
        len: usize,
    },
    Apply(Handle, Handle),
    Run(Handle),
    TreePut(Handle, usize, Handle),
    TablePut(Handle, usize, Entry),
    TableTake(Handle, usize),
    Length(Handle),
}

unsafe impl Sendable for Request {}

#[derive(Debug)]
#[repr(C)]
pub enum Entry {
    Null(usize),
    ReadOnly(Handle),
    ReadWrite(Handle),
}

#[derive(Debug)]
#[repr(C)]
pub enum Response {
    Ack,
    Handle(Handle),
    Type(DataType),
    Length(usize),
    Span { ptr: usize, len: usize },
    Entry(Entry),
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
