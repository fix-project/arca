#![no_std]
#![feature(asm_experimental_arch)]
extern crate alloc;
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

use alloc::vec::Vec;
use core::marker::PhantomData;
pub use macros::{num_fixutils_memories, num_fixutils_tables, procedure_entrypoint};
pub mod resource;
pub use resource::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    AllOccupied, // All memories/tables occupied
    Unavailable, // Resource unavailable
    GrowFailed,  // Memory/Table growth failed
    OutOfBounds, // Memory/Table access out of bounds
}

/// An operation that resolves to a WebAssembly externref
pub trait Resolve: Copy {
    /// Emits instructions for the operation, leaving the produced externref on the operand stack
    ///
    /// # Safety
    ///
    /// Caller must consume the externref before the enclosing function returns
    unsafe fn resolve(self);

    fn len(self) -> usize {
        let length: usize;
        unsafe {
            self.resolve();
            core::arch::asm!(
                "call fixpoint_len",
                "local.set {length}",
                length = out(local) length,
            );
        }
        length
    }

    fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Allocates a new memory to copy bytes from the blob this operation resolves to
    fn to_bytes(self) -> Result<Vec<u8>, Error> {
        Memory::from_handle(self)?.to_bytes(self.len())
    }

    /// Allocates a new table to copy elements from the tree this operation resolves to
    fn to_entries(self) -> Result<Vec<TableGet<'static>>, Error> {
        Table::from_handle(self)?.to_entries(self.len())
    }
}

pub fn from_bytes(bytes: &[u8]) -> Result<CreateBlob<'static>, Error> {
    Memory::from_bytes(bytes)?.to_handle(bytes.len())
}

pub fn from_entries<T: Resolve>(entries: &[T]) -> Result<CreateTree<'static>, Error> {
    Table::from_entries(entries)?.to_handle(entries.len())
}

// Resolves to the procedure's input combination
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Combination;

// Resolves to an entry in a table
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableGet<'a> {
    pub(crate) table_index: u32,
    pub(crate) entry_index: usize,
    pub(crate) source: PhantomData<&'a Table>, // Keeps the source borrowed
}

// Resolves to externref returned by fixshell create_blob() over first `length` bytes of the memory
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreateBlob<'a> {
    pub(crate) memory_index: u32,
    pub(crate) length: usize,
    pub(crate) source: PhantomData<&'a Memory>, // Keeps the source borrowed
}

// Resolves to externref returned by fixshell create_tree() over first `length` elements of the table
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreateTree<'a> {
    pub(crate) table_index: u32,
    pub(crate) length: usize,
    pub(crate) source: PhantomData<&'a Table>, // Keeps the source borrowed
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreateRef<T>(pub T);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identification<T>(pub T);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Application<T>(pub T);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection<T>(pub T);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrictEncode<T>(pub T);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShallowEncode<T>(pub T);

impl Resolve for Combination {
    #[inline(always)]
    unsafe fn resolve(self) {
        unsafe { core::arch::asm!("global.get combination_global") }
    }
}

impl Resolve for TableGet<'_> {
    #[inline(always)]
    unsafe fn resolve(self) {
        unsafe {
            core::arch::asm!(
                "local.get {table_index}",
                "local.get {entry_index}",
                "call wasm_table_get",
                table_index = in(local) self.table_index,
                entry_index = in(local) self.entry_index,
            )
        }
    }
}

impl Resolve for CreateBlob<'_> {
    #[inline(always)]
    unsafe fn resolve(self) {
        unsafe {
            core::arch::asm!(
                "local.get {memory_index}",
                "local.get {length}",
                "call fixpoint_create_blob",
                memory_index = in(local) self.memory_index,
                length = in(local) self.length,
            )
        }
    }
}

impl Resolve for CreateTree<'_> {
    #[inline(always)]
    unsafe fn resolve(self) {
        unsafe {
            core::arch::asm!(
                "local.get {table_index}",
                "local.get {length}",
                "call fixpoint_create_tree",
                table_index = in(local) self.table_index,
                length = in(local) self.length,
            )
        }
    }
}

impl<T: Resolve> Resolve for CreateRef<T> {
    #[inline(always)]
    unsafe fn resolve(self) {
        unsafe {
            self.0.resolve();
            core::arch::asm!("call fixpoint_create_ref");
        }
    }
}

impl<T: Resolve> Resolve for Identification<T> {
    #[inline(always)]
    unsafe fn resolve(self) {
        unsafe {
            self.0.resolve();
            core::arch::asm!("call fixpoint_create_identification_thunk");
        }
    }
}

impl<T: Resolve> Resolve for Application<T> {
    #[inline(always)]
    unsafe fn resolve(self) {
        unsafe {
            self.0.resolve();
            core::arch::asm!("call fixpoint_create_application_thunk");
        }
    }
}

impl<T: Resolve> Resolve for Selection<T> {
    #[inline(always)]
    unsafe fn resolve(self) {
        unsafe {
            self.0.resolve();
            core::arch::asm!("call fixpoint_create_selection_thunk");
        }
    }
}

impl<T: Resolve> Resolve for StrictEncode<T> {
    #[inline(always)]
    unsafe fn resolve(self) {
        unsafe {
            self.0.resolve();
            core::arch::asm!("call fixpoint_create_strict_encode");
        }
    }
}

impl<T: Resolve> Resolve for ShallowEncode<T> {
    #[inline(always)]
    unsafe fn resolve(self) {
        unsafe {
            self.0.resolve();
            core::arch::asm!("call fixpoint_create_shallow_encode");
        }
    }
}

// A generalized chain of operations with shape unknown at compile time
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandleOp<'a> {
    Combination,
    TableGet(TableGet<'a>),
    CreateBlob(CreateBlob<'a>),
    CreateTree(CreateTree<'a>),
    CreateRef(&'a HandleOp<'a>),
    Identification(&'a HandleOp<'a>),
    Application(&'a HandleOp<'a>),
    Selection(&'a HandleOp<'a>),
    StrictEncode(&'a HandleOp<'a>),
    ShallowEncode(&'a HandleOp<'a>),
}

impl Resolve for HandleOp<'_> {
    #[inline(always)]
    unsafe fn resolve(self) {
        unsafe {
            core::arch::asm!(
                "local.get {operation}",
                "call wasm_resolve",
                operation = in(local) (&raw const self) as usize,
            )
        }
    }
}

impl From<Combination> for HandleOp<'_> {
    fn from(_: Combination) -> Self {
        HandleOp::Combination
    }
}

impl<'a> From<TableGet<'a>> for HandleOp<'a> {
    fn from(operation: TableGet<'a>) -> Self {
        HandleOp::TableGet(operation)
    }
}

impl<'a> From<CreateBlob<'a>> for HandleOp<'a> {
    fn from(operation: CreateBlob<'a>) -> Self {
        HandleOp::CreateBlob(operation)
    }
}

impl<'a> From<CreateTree<'a>> for HandleOp<'a> {
    fn from(operation: CreateTree<'a>) -> Self {
        HandleOp::CreateTree(operation)
    }
}

/// Sets index `entry_index` of `table_index` with externref resolved from `operation`
///
/// # Safety
///
/// `table_index` must be in Table::RESOURCE_INDICES and `entry_index` < table size
#[inline(always)]
pub unsafe fn table_set<T: Resolve>(table_index: u32, entry_index: usize, operation: T) {
    unsafe {
        operation.resolve();
        core::arch::asm!(
            "local.get {table_index}",
            "local.get {entry_index}",
            "call wasm_table_set",
            table_index = in(local) table_index,
            entry_index = in(local) entry_index,
        );
    }
}

/// Calls fixshell attach_blob() with externref resolved from `operation`
///
/// # Safety
///
/// `memory_index` must be in Memory::RESOURCE_INDICES and `operation` must resolve to blob
#[inline(always)]
pub unsafe fn attach_blob<T: Resolve>(memory_index: u32, operation: T) {
    unsafe {
        operation.resolve();
        core::arch::asm!(
            "local.get {memory_index}",
            "call fixpoint_attach_blob",
            memory_index = in(local) memory_index,
        );
    }
}

/// Calls fixshell attach_tree with externref resolved from `operation`
///
/// # Safety
///
/// `table_index` must be in Table::RESOURCE_INDICES and `operation` must resolve to tree
#[inline(always)]
pub unsafe fn attach_tree<T: Resolve>(table_index: u32, operation: T) {
    unsafe {
        operation.resolve();
        core::arch::asm!(
            "local.get {table_index}",
            "call fixpoint_attach_tree",
            table_index = in(local) table_index,
        );
    }
}

#[macro_export]
macro_rules! declare_wasm {
    () => {
        r#"
.globaltype combination_global, externref
.functype wasm_resolve (i32) -> (externref)

# fixpoint module imports
.functype fixpoint_create_blob (i32, i32) -> (externref)
.import_module fixpoint_create_blob, fixpoint
.import_name fixpoint_create_blob, create_blob

.functype fixpoint_create_tree (i32, i32) -> (externref)
.import_module fixpoint_create_tree, fixpoint
.import_name fixpoint_create_tree, create_tree

.functype fixpoint_create_ref (externref) -> (externref)
.import_module fixpoint_create_ref, fixpoint
.import_name fixpoint_create_ref, create_ref

.functype fixpoint_create_identification_thunk (externref) -> (externref)
.import_module fixpoint_create_identification_thunk, fixpoint
.import_name fixpoint_create_identification_thunk, create_identification_thunk

.functype fixpoint_create_application_thunk (externref) -> (externref)
.import_module fixpoint_create_application_thunk, fixpoint
.import_name fixpoint_create_application_thunk, create_application_thunk

.functype fixpoint_create_selection_thunk (externref) -> (externref)
.import_module fixpoint_create_selection_thunk, fixpoint
.import_name fixpoint_create_selection_thunk, create_selection_thunk

.functype fixpoint_create_strict_encode (externref) -> (externref)
.import_module fixpoint_create_strict_encode, fixpoint
.import_name fixpoint_create_strict_encode, create_strict_encode

.functype fixpoint_create_shallow_encode (externref) -> (externref)
.import_module fixpoint_create_shallow_encode, fixpoint
.import_name fixpoint_create_shallow_encode, create_shallow_encode

.functype fixpoint_len (externref) -> (i32)
.import_module fixpoint_len, fixpoint
.import_name fixpoint_len, len

.functype fixpoint_attach_blob (externref, i32) -> ()
.import_module fixpoint_attach_blob, fixpoint
.import_name fixpoint_attach_blob, attach_blob

.functype fixpoint_attach_tree (externref, i32) -> ()
.import_module fixpoint_attach_tree, fixpoint
.import_name fixpoint_attach_tree, attach_tree
"#
    };
}

core::arch::global_asm!(
    declare_wasm!(),
    ".globl combination_global",
    "combination_global:",
    ".functype wasm_table_get (i32, i32) -> (externref)",
    ".functype wasm_table_set (externref, i32, i32) -> ()",
    ".functype _fixpoint_apply_inner () -> (i32)",
    // Puts input combination in global, calls procedure_entrypoint! macro's
    // _fixpoint_apply_inner, and resolves output HandleOp into externref
    ".globl _fixpoint_apply_wrapper",
    ".export_name _fixpoint_apply_wrapper, _fixpoint_apply",
    "_fixpoint_apply_wrapper:",
    ".functype _fixpoint_apply_wrapper (externref) -> (externref)",
    "local.get 0",
    "global.set combination_global",
    "call _fixpoint_apply_inner", // () -> *const HandleOp
    "call wasm_resolve",          // *const HandleOp -> externref
    "end_function",
    /*
     * wasm_resolve(operation: *const HandleOp) -> externref
     *
     * Resolves received HandleOp pointer with layout: operation type in first 4 bytes,
     * previous operation pointer or producer first argument in middle 4 bytes, and producer second argument in last 4 bytes
     */
    ".globl wasm_resolve",
    "wasm_resolve:",
    ".functype wasm_resolve (i32) -> (externref)",
    // One block per operation type plus one for default
    "block",
    "block",
    "block",
    "block",
    "block",
    "block",
    "block",
    "block",
    "block",
    "block",
    "block",
    "local.get 0",
    "i32.load 0", // Operation type
    "br_table {0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10}",
    "end_block",
    // 0: Combination -> input combination
    "global.get combination_global",
    "return",
    "end_block",
    // 1: TableGet { table_index, entry_index } -> wasm_table_get(table_index, entry_index)
    "local.get 0",
    "i32.load 4", // table_index
    "local.get 0",
    "i32.load 8", // entry_index
    "call wasm_table_get",
    "return",
    "end_block",
    // 2: CreateBlob { memory_index, length } -> fixpoint_create_blob(memory_index, length)
    "local.get 0",
    "i32.load 4", // memory_index
    "local.get 0",
    "i32.load 8", // length
    "call fixpoint_create_blob",
    "return",
    "end_block",
    // 3: CreateTree { table_index, length } -> fixpoint_create_tree(table_index, length)
    "local.get 0",
    "i32.load 4", // table_index
    "local.get 0",
    "i32.load 8", // length
    "call fixpoint_create_tree",
    "return",
    "end_block",
    // 4: CreateRef(operation: *const HandleOp) -> fixpoint_create_ref(wasm_resolve(operation))
    "local.get 0",
    "i32.load 4", // pointer to operation
    "call wasm_resolve",
    "call fixpoint_create_ref",
    "return",
    "end_block",
    // 5: Identification(operation: *const HandleOp) -> fixpoint_create_identification_thunk(wasm_resolve(operation))
    "local.get 0",
    "i32.load 4", // pointer to operation
    "call wasm_resolve",
    "call fixpoint_create_identification_thunk",
    "return",
    "end_block",
    // 6: Application(operation: *const HandleOp) -> fixpoint_create_application_thunk(wasm_resolve(operation))
    "local.get 0",
    "i32.load 4", // pointer to operation
    "call wasm_resolve",
    "call fixpoint_create_application_thunk",
    "return",
    "end_block",
    // 7: Selection(operation: *const HandleOp) -> fixpoint_create_selection_thunk(wasm_resolve(operation))
    "local.get 0",
    "i32.load 4", // pointer to operation
    "call wasm_resolve",
    "call fixpoint_create_selection_thunk",
    "return",
    "end_block",
    // 8: StrictEncode(operation: *const HandleOp) -> fixpoint_create_strict_encode(wasm_resolve(operation))
    "local.get 0",
    "i32.load 4", // pointer to operation
    "call wasm_resolve",
    "call fixpoint_create_strict_encode",
    "return",
    "end_block",
    // 9: ShallowEncode(operation: *const HandleOp) -> fixpoint_create_shallow_encode(wasm_resolve(operation))
    "local.get 0",
    "i32.load 4", // pointer to operation
    "call wasm_resolve",
    "call fixpoint_create_shallow_encode",
    "return",
    "end_block",
    // Default
    "unreachable",
    "end_function",
    options(raw),
);

unsafe extern "C" {
    pub fn wasm_memory_read(memory_index: u32, destination: u32, length: usize);
    pub fn wasm_memory_write(memory_index: u32, source: u32, length: usize);
    pub fn wasm_memory_size(memory_index: u32) -> usize;
    pub fn wasm_memory_grow(memory_index: u32, num_pages: usize) -> usize;

    pub fn wasm_table_size(table_index: u32) -> usize;
    pub fn wasm_table_grow(table_index: u32, entries: usize) -> usize;
}
