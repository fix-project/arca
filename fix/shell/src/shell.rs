#![allow(
    improper_ctypes_definitions,
    reason = "userspace uses the System V SIMD ABI; vector layout is asserted in rt"
)]
use arca::{Blob, Entry, Function, Table, Value, Word};
use arca::{Runtime as _, Tuple};
use arcane::{
    __MODE_read_only, __MODE_read_write, __NR_length, __TYPE_table, arca_argument,
    arca_blob_create, arca_blob_read, arca_entry, arca_mmap, arca_mprotect, arca_table_map, arcad,
};

use core::arch::x86_64::*;
use core::ffi::{CStr, c_char, c_uint, c_void};
use core::simd::u8x32;
use fixhandle::*;
use fixtypes::{DataType, EncodeType, ValueType};

use user::ArcaError;
use user::Runtime;
use user::error::log as arca_log;
use user::error::log_int as arca_log_int;

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_create_blob_i32(val: u32) -> u8x32 {
    let bytes = val.to_le_bytes();
    unsafe { fixpoint_create_blob(&bytes) }
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_create_blob_i64(val: u64) -> u8x32 {
    let bytes = val.to_le_bytes();
    unsafe { fixpoint_create_blob(&bytes) }
}

/// Attaches a blob to a region of memory.  Returns the size (in bytes) of the mapped blob.
///
/// # Safety
///
/// `addr` must start a reserved memory slot containing `old_size` mapped bytes.
/// No Rust references may point into the slot.
#[inline]
pub unsafe extern "C" fn fixpoint_attach_blob(
    handle: u8x32,
    addr: *mut c_void,
    old_size: usize,
) -> usize {
    if !unsafe { fixpoint_is_blob_obj(handle) } {
        arca_log("attach_blob: handle does not refer to a BlobObject");
        panic!()
    }

    let result: Result<Value<Runtime>, ArcaError> = Function::symbolic("get_blob")
        .apply(Runtime::create_blob(handle.as_array()))
        .call_with_current_continuation()
        .try_into()
        .map_err(|_| ArcaError::BadType);

    let Ok(blob) = result else {
        arca_log("attach_blob: failed to get BlobData");
        panic!()
    };
    let len = unsafe { fixpoint_len(handle) };
    assert!(len <= crate::rt::SLOT_SIZE);
    unsafe { attach_data(&blob, addr.cast(), old_size, len) };
    len
}

/// Attaches a tree to a region of memory.  Returns the size (in elements) of the tree.
///
/// # Safety
///
/// `addr` must start a reserved table slot containing `old_size` mapped bytes.
/// No Rust references may point into the slot.
#[inline]
pub unsafe extern "C" fn fixpoint_attach_tree(
    handle: u8x32,
    addr: *mut c_void,
    old_size: usize,
) -> usize {
    if !unsafe { fixpoint_is_object(handle) && fixpoint_is_tree(handle) } {
        arca_log("attach_tree: handle does not refer to a TreeObject");
        panic!()
    }

    let result: Result<Value<Runtime>, ArcaError> = Function::symbolic("get_tree")
        .apply(Runtime::create_blob(handle.as_array()))
        .call_with_current_continuation()
        .try_into()
        .map_err(|_| ArcaError::BadType);

    let Ok(tree) = result else {
        arca_log("attach_tree: failed to get TreeData");
        panic!()
    };

    let len = unsafe { fixpoint_len(handle) };
    assert!(len <= crate::rt::MAX_TABLE_ELEMENTS as usize);
    let bytes = len * crate::rt::EXTERNREF_SIZE;
    unsafe { attach_data(&tree, addr.cast(), old_size, bytes) };
    len
}

pub fn backing_page(data: &Value<Runtime>, mut offset: usize) -> (arca::Page<Runtime>, usize) {
    let mut table = match data {
        Value::Table(table) => {
            assert!(table.len() <= 1 << 30);
            table.clone()
        }
        Value::Tuple(chunks) => {
            assert!(chunks.len() <= 4);
            let table: Table<Runtime> = chunks
                .get(offset >> 30)
                .try_into()
                .expect("data chunk is not a Table");
            assert_eq!(table.len(), 1 << 30);
            offset &= (1 << 30) - 1;
            table
        }
        _ => panic!("expected page-backed data"),
    };
    loop {
        let span = table.len() / 512;
        match table.get(offset / span).expect("attachment mapping") {
            Entry::ROTable(child) | Entry::RWTable(child) => {
                table = child;
                offset %= span;
            }
            Entry::ROPage(page) => return (page, offset % span),
            _ => panic!("attachment is not immutable page backing"),
        }
    }
}

unsafe fn attach_data(data: &Value<Runtime>, addr: *mut u8, old_size: usize, len: usize) {
    assert!(
        matches!(data, Value::Table(_) | Value::Tuple(_)),
        "expected page-backed data"
    );
    let size = len.next_multiple_of(crate::rt::PAGE_SIZE as usize);
    unsafe { crate::rt::replace_region(addr, old_size, 0) };
    for offset in (0..size).step_by(4096) {
        let page = if offset < len {
            let (page, position) = backing_page(data, offset);
            assert_eq!(position, 0);
            assert_eq!(page.len(), 4096);
            if offset + 4096 > len {
                let position = len - offset;
                let mut padding = [0; 4096];
                assert_eq!(
                    page.read(position, &mut padding[..4096 - position]),
                    4096 - position
                );
                assert!(padding[..4096 - position].iter().all(|byte| *byte == 0));
            }
            page
        } else {
            arca::Page::<Runtime>::new(4096)
        };
        let mut entry = arca_entry {
            mode: __MODE_read_only,
            datatype: arcane::__TYPE_page,
            data: page.into_inner().into_raw() as usize,
        };
        assert_eq!(unsafe { arca_mmap(addr.add(offset).cast(), &mut entry) }, 0);
    }
}

pub unsafe fn detach_data(addr: *mut u8, size: usize, copy: bool) {
    if copy {
        for offset in (0..size).step_by(4096) {
            assert_eq!(
                unsafe { arca_mprotect(addr.add(offset).cast(), __MODE_read_write as i32) },
                0
            );
        }
    } else {
        assert!(unsafe { crate::rt::map_region(addr, size, arcane::__MODE_none) }.is_some());
    }
}

/// Creates a blob from a region of memory.  Returns the handle,
///
/// # Safety
///
/// [addr] must refer to an region of memory which is large enough for the specified [len];  
#[inline]
pub unsafe fn fixpoint_create_blob(slice: &[u8]) -> u8x32 {
    unsafe { fixpoint_create_blob_from_memory(slice.as_ptr(), slice.len()) }
}

/// # Safety
/// The region at `data` must contain `len` readable bytes.
#[inline]
pub unsafe extern "C" fn fixpoint_create_blob_from_memory(data: *const u8, len: usize) -> u8x32 {
    let raw = unsafe { arca_blob_create(data, len) };
    assert!(raw >= 0);
    let blob = Blob::<Runtime>::from_inner(user::Ref::from_raw(u32::try_from(raw).unwrap()));
    receive(Function::symbolic("create_blob").apply(blob))
}

/// Creates a tree from a region of memory.  Returns the handle,
///
/// # Safety
///
/// `slice` must be a prefix of a reserved externref table slot, containing
/// 32-byte handles supplied by the runtime.
#[inline]
pub unsafe fn fixpoint_create_tree(slice: &[u8]) -> u8x32 {
    assert!(slice.len().is_multiple_of(HANDLE_SIZE));
    let backing = unsafe { snapshot_tree(slice.as_ptr(), slice.len()) };
    receive(
        Function::symbolic("create_tree")
            .apply(backing)
            .apply(Word::<Runtime>::new(slice.len() as u64)),
    )
}

unsafe fn snapshot_tree(addr: *const u8, len: usize) -> Value<Runtime> {
    const CHUNK: usize = 1 << 30;
    assert_eq!(addr as usize % CHUNK, 0);
    let mut chunks = Tuple::<Runtime>::new(len.div_ceil(CHUNK).max(1));
    for index in 0..chunks.len() {
        let used = (len - index * CHUNK).min(CHUNK);
        if used == 0 {
            chunks.set(index, Table::<Runtime>::new(0));
            continue;
        }
        let position = unsafe { addr.add(index * CHUNK) };
        let mut raw = arca_entry {
            mode: arcane::__MODE_none,
            datatype: arcane::__TYPE_null,
            data: CHUNK,
        };
        assert_eq!(
            unsafe { arca_mmap(position.cast_mut().cast(), &mut raw) },
            0
        );
        let reference = user::Ref::from_raw(u32::try_from(raw.data).unwrap());
        let mut source = match (raw.datatype, raw.mode) {
            (arcane::__TYPE_page, arcane::__MODE_read_only) => {
                Entry::ROPage(arca::Page::from_inner(reference))
            }
            (arcane::__TYPE_page, arcane::__MODE_read_write) => {
                Entry::RWPage(arca::Page::from_inner(reference))
            }
            (arcane::__TYPE_table, arcane::__MODE_read_only) => {
                Entry::ROTable(Table::from_inner(reference))
            }
            (arcane::__TYPE_table, arcane::__MODE_read_write) => {
                Entry::RWTable(Table::from_inner(reference))
            }
            _ => panic!("missing tree mapping"),
        };
        let Entry::ROTable(snapshot) = snapshot_tree_entry(&mut source, used) else {
            panic!("invalid tree backing")
        };
        raw.data = match source {
            Entry::ROPage(page) | Entry::RWPage(page) => page.into_inner().into_raw() as usize,
            Entry::ROTable(table) | Entry::RWTable(table) => table.into_inner().into_raw() as usize,
            Entry::Null(_) => unreachable!(),
        };
        assert_eq!(
            unsafe { arca_mmap(position.cast_mut().cast(), &mut raw) },
            0
        );
        chunks.set(index, snapshot);
    }
    if chunks.len() == 1 {
        chunks.take(0)
    } else {
        chunks.into()
    }
}

fn snapshot_tree_entry(source: &mut Entry<Runtime>, len: usize) -> Entry<Runtime> {
    match source {
        Entry::ROTable(table) | Entry::RWTable(table) => {
            let mut result = Table::<Runtime>::new(table.len());
            let span = table.len() / 512;
            for index in 0..len.div_ceil(span) {
                let mut entry = table.take(index).expect("tree mapping");
                result
                    .set(
                        index,
                        snapshot_tree_entry(&mut entry, (len - index * span).min(span)),
                    )
                    .expect("tree mapping");
                table.set(index, entry).expect("tree mapping");
            }
            Entry::ROTable(result)
        }
        Entry::ROPage(page) | Entry::RWPage(page) => {
            if page.len() == 4096 {
                return Entry::ROPage(snapshot_tree_page(page, 0, len));
            }
            let mut result = Table::<Runtime>::new(page.len());
            for offset in (0..len).step_by(4096) {
                let snapshot = snapshot_tree_page(page, offset, (len - offset).min(4096));
                assert!(result.map(offset, Entry::ROPage(snapshot)).is_ok());
            }
            Entry::ROTable(result)
        }
        Entry::Null(_) => panic!("missing tree mapping"),
    }
}

fn snapshot_tree_page(
    page: &arca::Page<Runtime>,
    offset: usize,
    len: usize,
) -> arca::Page<Runtime> {
    if page.len() == 4096 {
        return page.clone();
    }
    let mut bytes = [0; 4096];
    assert_eq!(page.read(offset, &mut bytes[..len]), len);
    let mut snapshot = arca::Page::<Runtime>::new(4096);
    assert_eq!(snapshot.write(0, &bytes), bytes.len());
    snapshot
}

/// Creates a tag from a region of memory.  Returns the handle,
///
/// # Safety
///
/// [addr] must refer to an region of memory which is large enough for the specified [len];  
/// Each entry of the tree takes 32 bytes.
#[inline]
pub unsafe fn fixpoint_create_tag(slice: &[u8], author_depth: u32) -> u8x32 {
    assert!(slice.len() >= 32);
    let backing = unsafe { snapshot_tree(slice.as_ptr(), slice.len()) };
    receive(
        Function::symbolic("create_tag")
            .apply(backing)
            .apply(Word::<Runtime>::new(slice.len() as u64))
            .apply(Word::<Runtime>::new(author_depth as u64)),
    )
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_get_type(bytes: u8x32) -> ValueType {
    let handle = unsafe { Handle::parse_native(bytes) }.expect("get_type: invalid handle");
    match handle.view() {
        HandleView::Object(_) => ValueType::Object,
        HandleView::Ref(_) => ValueType::Ref,
        HandleView::Thunk(_) => ValueType::Thunk,
        HandleView::Encode(_) => ValueType::Encode,
    }
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_get_data_type(bytes: u8x32) -> DataType {
    let handle = unsafe { Handle::parse_native(bytes) }.expect("get_data_type: invalid handle");
    let object = match handle.view() {
        HandleView::Object(object) => *object,
        HandleView::Ref(reference) => reference.object_descriptor(),
        _ => panic!("get_data_type: expected Object or Ref"),
    };
    match object.view() {
        ObjectView::Blob(_) => DataType::Blob,
        ObjectView::Tree(_) => DataType::Tree,
    }
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_get_encode_type(bytes: u8x32) -> EncodeType {
    let handle = unsafe { Handle::parse_native(bytes) }.expect("get_encode_type: invalid handle");
    let HandleView::Encode(encode) = handle.view() else {
        panic!("get_encode_type: expected Encode");
    };
    match encode.view() {
        EncodeView::Strict(_) => EncodeType::Strict,
        EncodeView::Shallow(_) => EncodeType::Shallow,
    }
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_is_blob_obj(bytes: u8x32) -> bool {
    unsafe { Handle::parse_native(bytes) }
        .and_then(fixhandle::Blob::try_from)
        .is_ok()
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_is_blob(bytes: u8x32) -> bool {
    unsafe { Handle::parse_native(bytes) }
        .and_then(data_reference)
        .and_then(|r| fixhandle::Blob::try_from(r.object_descriptor().handle()))
        .is_ok()
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_is_tree(bytes: u8x32) -> bool {
    unsafe { Handle::parse_native(bytes) }
        .and_then(data_reference)
        .and_then(|r| TreeRef::try_from(r.handle()))
        .is_ok()
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_is_object(bytes: u8x32) -> bool {
    unsafe { Handle::parse_native(bytes) }
        .and_then(Object::try_from)
        .is_ok()
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_is_data(bytes: u8x32) -> bool {
    unsafe { Handle::parse_native(bytes) }
        .and_then(data_reference)
        .is_ok()
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_is_tag(bytes: u8x32) -> bool {
    unsafe { Handle::parse_native(bytes) }
        .and_then(Tree::try_from)
        .is_ok_and(|t| t.is_tag())
}

fn data_reference(handle: Handle) -> Result<fixhandle::Ref, fixhandle::Error> {
    match handle.view() {
        HandleView::Object(object) => Ok(object.into_ref()),
        HandleView::Ref(reference) => Ok(*reference),
        _ => Err(fixhandle::Error::WrongType),
    }
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_is_comparable(bytes: u8x32) -> bool {
    unsafe { Handle::parse_native(bytes) }
        .expect("is_comparable: invalid handle")
        .is_comparable()
}

/// # Safety
/// `handle` must have been supplied by the runtime.
pub unsafe fn fixpoint_unsound_handle_bytes(handle: u8x32) -> u8x32 {
    let privileged: Word<Runtime> = Function::symbolic("is_privileged")
        .call_with_current_continuation()
        .try_into()
        .expect("is_privileged: return type is not a word");
    assert_eq!(privileged.read(), 1, "handle bytes require privilege");
    receive(
        Function::symbolic("unsound_handle_bytes").apply(Runtime::create_blob(handle.as_array())),
    )
}

/// # Safety
/// Both operands must be Handles supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_equals(lhs: u8x32, rhs: u8x32) -> bool {
    assert!(
        unsafe { fixpoint_is_comparable(lhs) && fixpoint_is_comparable(rhs) },
        "equals: non-comparable operand"
    );
    let result: Word<Runtime> = Function::symbolic("equals")
        .apply(Runtime::create_blob(lhs.as_array()))
        .apply(Runtime::create_blob(rhs.as_array()))
        .call_with_current_continuation()
        .try_into()
        .expect("equals: return type is not a word");

    let result = result.read();
    result == 1
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_create_ref(bytes: u8x32) -> u8x32 {
    Object::try_from(unsafe { Handle::parse_native(bytes) }.expect("create_ref: invalid handle"))
        .expect("create_ref: expected object")
        .into_ref()
        .handle()
        .into_native()
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_create_identification_thunk(bytes: u8x32) -> u8x32 {
    data_reference(unsafe { Handle::parse_native(bytes) }.expect("identification: invalid handle"))
        .expect("identification: expected object or ref")
        .identification()
        .handle()
        .into_native()
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_create_application_thunk(bytes: u8x32) -> u8x32 {
    let reference = data_reference(
        unsafe { Handle::parse_native(bytes) }.expect("application: invalid handle"),
    )
    .expect("application: expected object or ref");
    TreeRef::try_from(reference.handle())
        .expect("application: expected tree ref")
        .application()
        .handle()
        .into_native()
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_create_selection_thunk(bytes: u8x32) -> u8x32 {
    let reference =
        data_reference(unsafe { Handle::parse_native(bytes) }.expect("selection: invalid handle"))
            .expect("selection: expected object or ref");
    TreeRef::try_from(reference.handle())
        .expect("selection: expected tree ref")
        .selection()
        .handle()
        .into_native()
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_create_strict_encode(bytes: u8x32) -> u8x32 {
    Thunk::try_from(unsafe { Handle::parse_native(bytes) }.expect("strict: invalid handle"))
        .expect("strict: expected thunk")
        .strict()
        .handle()
        .into_native()
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_create_shallow_encode(bytes: u8x32) -> u8x32 {
    Thunk::try_from(unsafe { Handle::parse_native(bytes) }.expect("shallow: invalid handle"))
        .expect("shallow: expected thunk")
        .shallow()
        .handle()
        .into_native()
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_len(bytes: u8x32) -> usize {
    data_reference(unsafe { Handle::parse_native(bytes) }.expect("len: invalid handle"))
        .expect("len: expected Object or Ref")
        .handle()
        .checked_len()
        .expect("len: handle length exceeds address space")
}

// void __assert_fail(const char * assertion, const char * file, unsigned int line, const char * function);
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __assert_fail(
    assertion: *const c_char,
    file: *const c_char,
    line: c_uint,
    function: *const c_char,
) -> ! {
    unsafe {
        let assertion = CStr::from_ptr(assertion).display();
        let file = CStr::from_ptr(file).display();
        let function = CStr::from_ptr(function).display();
        panic!("assertion failed at {file}:{line} in {function}: {assertion}");
    }
}

pub fn receive(function: Function<Runtime>) -> u8x32 {
    let result: Blob<Runtime> = function
        .call_with_current_continuation()
        .try_into()
        .expect("expected a handle");
    let mut bytes = [0; 32];
    assert_eq!(result.len(), bytes.len());
    assert_eq!(result.read(0, &mut bytes), bytes.len());
    u8x32::from_array(bytes)
}
