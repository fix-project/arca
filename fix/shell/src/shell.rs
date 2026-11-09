use crate::continuation::receive;
use arca::{Blob, Entry, Table, Value, Word};
use arca::{Runtime as _, Tuple};
use arcane::{
    __MODE_read_only, __MODE_read_write, __NR_length, __TYPE_table, arca_argument,
    arca_blob_create, arca_blob_read, arca_entry, arca_mmap, arca_mprotect, arca_table_map, arcad,
};

use crate::handles::Reference;
use crate::handles::{self, Query};
use core::ffi::{CStr, c_char, c_uint, c_void};
use fixhandle::HANDLE_SIZE;
use fixtypes::{DataType, EncodeType, ThunkType, ValueType};

use user::ArcaError;
use user::Runtime;
use user::error::log as arca_log;
use user::error::log_int as arca_log_int;

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_create_blob_i32(result: Reference, val: u32) {
    let bytes = val.to_le_bytes();
    unsafe { fixpoint_create_blob(result, &bytes) }
}

/// # Safety
/// `bytes` must encode a Handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_create_blob_i64(result: Reference, val: u64) {
    let bytes = val.to_le_bytes();
    unsafe { fixpoint_create_blob(result, &bytes) }
}

/// Attaches a blob to a region of memory.  Returns the size (in bytes) of the mapped blob.
///
/// # Safety
///
/// `addr` must start a reserved memory slot containing `old_size` mapped bytes.
/// No Rust references may point into the slot.
#[inline]
pub unsafe extern "C" fn fixpoint_attach_blob(
    handle: Reference,
    addr: *mut c_void,
    old_size: usize,
) -> usize {
    crate::locals::lift(&[handle]);
    if !unsafe { fixpoint_is_blob_obj(handle) } {
        arca_log("attach_blob: handle does not refer to a BlobObject");
        panic!()
    }

    if unsafe { handles::query(handle, handles::Query::Literal) } != 0 {
        let len = unsafe { fixpoint_len(handle) };
        unsafe {
            crate::rt::replace_region(addr.cast(), old_size, 0);
            if len != 0 {
                crate::rt::map_region(
                    addr.cast(),
                    len.next_multiple_of(crate::rt::PAGE_SIZE as usize),
                    __MODE_read_write,
                )
                .expect("inline Blob mapping failed");
                crate::rt::copy_guest_bytes(handles::address(handle), addr as usize, len);
                assert_eq!(arca_mprotect(addr, __MODE_read_only as i32), 0);
            }
        }
        return len;
    }

    let result: Result<Value<Runtime>, ArcaError> = crate::continuation::operation("get_blob")
        .blob(&handles::bytes(handle))
        .call_with_current_continuation()
        .try_into()
        .map_err(|_| ArcaError::BadType);

    let Ok(blob) = result else {
        arca_log("attach_blob: failed to get BlobData");
        panic!()
    };
    let len = unsafe { fixpoint_len(handle) };
    assert!(len <= crate::rt::PAYLOAD_SIZE);
    unsafe {
        attach_data(
            blob,
            addr.cast(),
            old_size,
            len,
            len.next_multiple_of(crate::rt::PAGE_SIZE as usize),
        )
    };
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
    handle: Reference,
    addr: *mut c_void,
    old_size: usize,
) -> usize {
    if !unsafe { fixpoint_is_object(handle) && fixpoint_is_tree(handle) } {
        arca_log("attach_tree: handle does not refer to a TreeObject");
        panic!()
    }

    let len = unsafe { fixpoint_len(handle) };
    if len == 0 {
        unsafe { crate::rt::replace_region(addr.cast(), old_size, 0) };
        return 0;
    }
    crate::locals::lift(&[handle]);

    let result: Result<Value<Runtime>, ArcaError> = crate::continuation::operation("get_tree")
        .blob(&handles::bytes(handle))
        .call_with_current_continuation()
        .try_into()
        .map_err(|_| ArcaError::BadType);

    let Ok(tree) = result else {
        arca_log("attach_tree: failed to get TreeData");
        panic!()
    };

    assert!(len <= crate::rt::MAX_TABLE_ELEMENTS as usize);
    let bytes = len * HANDLE_SIZE;
    unsafe {
        attach_data(
            tree,
            addr.cast(),
            old_size,
            bytes,
            bytes.next_multiple_of(4096),
        )
    };
    len
}

pub unsafe extern "C" fn fixpoint_attach_tree_bytes(
    handle: Reference,
    addr: *mut c_void,
    old_size: usize,
) -> usize {
    require_privilege();
    assert!(unsafe { fixpoint_is_object(handle) && fixpoint_is_tree(handle) });
    let bytes = unsafe { fixpoint_len(handle) }
        .checked_mul(HANDLE_SIZE)
        .expect("Tree byte length overflow");
    if bytes == 0 {
        unsafe { crate::rt::replace_region(addr.cast(), old_size, 0) };
        return 0;
    }
    crate::locals::lift(&[handle]);
    let tree: Value<Runtime> = crate::continuation::operation("get_tree")
        .blob(&handles::bytes(handle))
        .call_with_current_continuation();
    unsafe {
        attach_data(
            tree,
            addr.cast(),
            old_size,
            bytes,
            bytes.next_multiple_of(crate::rt::PAGE_SIZE as usize),
        )
    };
    bytes
}

unsafe fn attach_data(
    data: Value<Runtime>,
    addr: *mut u8,
    old_size: usize,
    len: usize,
    size: usize,
) {
    unsafe {
        crate::rt::replace_region(addr, old_size, 0);
        map_data(data, addr, len, size);
    }
}

pub(crate) unsafe fn map_data(data: Value<Runtime>, addr: *mut u8, len: usize, size: usize) {
    assert!(len <= size && size.is_multiple_of(4096));
    unsafe fn map(value: Value<Runtime>, address: *mut u8) {
        let (datatype, data) = match value {
            Value::Page(page) => {
                assert_eq!(page.len(), 4096);
                (arcane::__TYPE_page, page.into_inner().into_raw())
            }
            Value::Table(table) => (arcane::__TYPE_table, table.into_inner().into_raw()),
            _ => panic!("expected mapping"),
        };
        let mut entry = arca_entry {
            mode: __MODE_read_only,
            datatype,
            data: data as usize,
        };
        assert_eq!(unsafe { arca_mmap(address.cast(), &mut entry) }, 0);
        if entry.datatype != arcane::__TYPE_null {
            assert_eq!(unsafe { arcane::arca_drop(entry.data as i64) }, 0);
        }
    }
    if len != 0 {
        match data {
            Value::Tuple(mut chunks) => {
                assert!(chunks.len() <= 4);
                for index in 0..len.div_ceil(1 << 30) {
                    unsafe {
                        map(chunks.take(index), addr.add(index << 30));
                    }
                }
            }
            value => unsafe { map(value, addr) },
        }
    }
    let mapped = len.next_multiple_of(4096);
    if size > mapped {
        unsafe { crate::rt::map_region(addr.add(mapped), size - mapped, __MODE_read_only) }
            .expect("attachment padding failed");
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
pub unsafe fn fixpoint_create_blob(result: Reference, slice: &[u8]) {
    unsafe { fixpoint_create_blob_from_memory(result, slice.as_ptr(), slice.len()) }
}

/// # Safety
/// The region at `data` must contain `len` readable bytes.
#[inline]
pub unsafe extern "C" fn fixpoint_create_blob_from_memory(
    result: Reference,
    data: *const u8,
    len: usize,
) {
    if len == 0 {
        unsafe { core::ptr::write_bytes(result, 0, 1) };
        return;
    }
    if len <= 30 {
        let cell = handles::address(result);
        unsafe {
            core::ptr::write_bytes(cell as *mut u8, 0, HANDLE_SIZE);
            crate::rt::copy_guest_bytes(data as usize, cell, len);
            *((cell + 30) as *mut u8) = len as u8;
        }
        return;
    }
    let raw = unsafe { arca_blob_create(data, len) };
    assert!(raw >= 0);
    let blob = Blob::<Runtime>::from_inner(user::Ref::from_raw(u32::try_from(raw).unwrap()));
    unsafe { crate::locals::create(result, blob.into(), len, 0x800f, alloc::vec::Vec::new()) }
}

/// Creates a tree from a region of memory.  Returns the handle,
///
/// # Safety
///
/// `slice` must be a prefix of a reserved externref table slot, containing
/// packed handles supplied by the shell or runtime.
#[inline]
pub unsafe fn fixpoint_create_tree(result: Reference, slice: &[u8]) {
    assert!(slice.len().is_multiple_of(HANDLE_SIZE));
    let mut metadata = 0xa00fu16;
    let mut dependencies = alloc::vec::Vec::new();
    for offset in (0..slice.len()).step_by(HANDLE_SIZE) {
        let cell = slice.as_ptr() as usize + offset;
        let source = unsafe { handles::query_cell(cell, Query::LocalAddress) };
        if source != 0 {
            dependencies.push(source);
        }
        if unsafe { handles::query_cell(cell, Query::Comparable) } == 0 {
            metadata &= !4;
        }
        if unsafe { handles::query_cell(cell, Query::Value) } == 0 {
            metadata &= !8;
        }
    }
    let backing = unsafe { snapshot_tree(slice.as_ptr(), slice.len()) };
    unsafe { crate::locals::create(result, backing, slice.len(), metadata, dependencies) }
}

pub unsafe fn snapshot_tree(addr: *const u8, len: usize) -> Value<Runtime> {
    const CHUNK: usize = 1 << 30;
    assert_eq!(addr as usize % CHUNK, 0);
    if len == 0 {
        return arca::Page::<Runtime>::new(4096).into();
    }
    let mut chunks = Tuple::<Runtime>::new(len.div_ceil(CHUNK).max(1));
    for index in 0..chunks.len() {
        let used = (len - index * CHUNK).min(CHUNK);
        let position = unsafe { addr.add(index * CHUNK) };
        let mut raw = arca_entry {
            mode: arcane::__MODE_none,
            datatype: arcane::__TYPE_null,
            data: if len > CHUNK {
                CHUNK
            } else if used <= 4096 {
                4096
            } else if used <= 1 << 21 {
                1 << 21
            } else {
                CHUNK
            },
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
        let snapshot: Value<Runtime> = match snapshot_tree_entry(&mut source, used) {
            Entry::ROPage(page) => page.into(),
            Entry::ROTable(table) => table.into(),
            _ => unreachable!(),
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
            assert_eq!(page.len(), 4096);
            Entry::ROPage(page.clone())
        }
        Entry::Null(_) => panic!("missing tree mapping"),
    }
}

/// Creates a tag from a region of memory.  Returns the handle,
///
/// # Safety
///
/// [addr] must refer to an region of memory which is large enough for the specified [len];  
/// Entries are packed handles.
#[inline]
pub unsafe fn fixpoint_create_tag(result: Reference, slice: &[u8]) {
    crate::locals::lift_table(slice.as_ptr() as usize, slice.len() / HANDLE_SIZE);
    let width = HANDLE_SIZE;
    assert!(slice.len() >= width && slice.len().is_multiple_of(width));
    let backing = unsafe { snapshot_tree(slice.as_ptr(), slice.len()) };
    receive(
        result,
        crate::continuation::operation("create_tag")
            .apply(backing)
            .word((slice.len() / width) as u64),
    )
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_get_type(bytes: Reference) -> ValueType {
    match unsafe { handles::query(bytes, Query::Type) } {
        0 => ValueType::Object,
        1 => ValueType::Ref,
        2 => ValueType::Thunk,
        3 => ValueType::Encode,
        _ => panic!("invalid value type"),
    }
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_get_data_type(bytes: Reference) -> DataType {
    match unsafe { handles::query(bytes, Query::DataType) } {
        0 => DataType::Blob,
        1 => DataType::Tree,
        _ => panic!("get_data_type: expected Object or Ref"),
    }
}

/// # Safety
/// `handle` must be a handle supplied by the runtime.
pub unsafe fn fixpoint_get_thunk_type(handle: Reference) -> ThunkType {
    require_privilege();
    match unsafe { handles::query(handle, Query::ThunkType) } {
        0 => ThunkType::Application,
        1 => ThunkType::Selection,
        2 => ThunkType::Identification,
        _ => panic!("get_thunk_type: expected Thunk or Encode"),
    }
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_get_encode_type(bytes: Reference) -> EncodeType {
    match unsafe { handles::query(bytes, Query::EncodeType) } {
        0 => EncodeType::Strict,
        1 => EncodeType::Shallow,
        _ => panic!("get_encode_type: expected Encode"),
    }
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_is_blob_obj(bytes: Reference) -> bool {
    unsafe { fixpoint_is_object(bytes) && fixpoint_is_blob(bytes) }
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_is_blob(bytes: Reference) -> bool {
    (unsafe { handles::query(bytes, Query::DataType) }) == 0
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_is_tree(bytes: Reference) -> bool {
    (unsafe { handles::query(bytes, Query::DataType) }) == 1
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_is_object(bytes: Reference) -> bool {
    (unsafe { handles::query(bytes, Query::Type) }) == 0
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_is_data(bytes: Reference) -> bool {
    (unsafe { handles::query(bytes, Query::Type) }) < 2
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_is_tag(bytes: Reference) -> bool {
    (unsafe { handles::query(bytes, Query::Tag) }) == 1
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_is_comparable(bytes: Reference) -> bool {
    (unsafe { handles::query(bytes, Query::Comparable) }) == 1
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_is_value(bytes: Reference) -> bool {
    (unsafe { handles::query(bytes, Query::Value) }) == 1
}

static mut PRIVILEGED: bool = false;

pub(crate) fn clear_privilege_cache() {
    unsafe { PRIVILEGED = false };
}

pub(crate) fn require_privilege() {
    if unsafe { PRIVILEGED } {
        return;
    }
    let privileged: Word<Runtime> = crate::continuation::operation("is_privileged")
        .call_with_current_continuation()
        .try_into()
        .expect("is_privileged: return type is not a word");
    assert_eq!(privileged.read(), 1, "operation requires privilege");
    unsafe { PRIVILEGED = true };
}

/// # Safety
/// `handle` must have been supplied by the runtime.
pub unsafe fn fixpoint_unsound_handle_bytes(result: Reference, handle: Reference) {
    crate::locals::lift(&[handle]);
    receive(
        result,
        crate::continuation::operation("unsound_handle_bytes").blob(&handles::bytes(handle)),
    )
}

/// # Safety
/// Both operands must be Handles supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_equals(lhs: Reference, rhs: Reference) -> bool {
    crate::locals::lift(&[lhs, rhs]);
    assert!(
        unsafe { fixpoint_is_comparable(lhs) && fixpoint_is_comparable(rhs) },
        "equals: non-comparable operand"
    );
    let result: Word<Runtime> = crate::continuation::operation("equals")
        .blob(&handles::bytes(lhs))
        .blob(&handles::bytes(rhs))
        .call_with_current_continuation()
        .try_into()
        .expect("equals: return type is not a word");

    let result = result.read();
    result == 1
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_create_ref(result: Reference, bytes: Reference) {
    assert!(
        unsafe { fixpoint_is_object(bytes) },
        "create_ref: expected Object"
    );
    unsafe { handles::transform(result, bytes, 0x10) }
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_create_identification_thunk(result: Reference, bytes: Reference) {
    assert!(
        unsafe { fixpoint_is_data(bytes) },
        "identification: expected Object or Ref"
    );
    unsafe { handles::transform(result, bytes, 0x18) }
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_create_application_thunk(result: Reference, bytes: Reference) {
    assert!(
        unsafe { fixpoint_is_tree(bytes) },
        "application: expected Tree Object or Ref"
    );
    unsafe { handles::transform(result, bytes, 0x14) }
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_create_selection_thunk(result: Reference, bytes: Reference) {
    assert!(
        unsafe { fixpoint_is_tree(bytes) },
        "selection: expected Tree Object or Ref"
    );
    unsafe { handles::transform(result, bytes, 0x1c) }
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_create_strict_encode(result: Reference, bytes: Reference) {
    assert_eq!(
        unsafe { fixpoint_get_type(bytes) },
        ValueType::Thunk,
        "strict: expected Thunk"
    );
    unsafe { handles::transform(result, bytes, 1) }
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_create_shallow_encode(result: Reference, bytes: Reference) {
    assert_eq!(
        unsafe { fixpoint_get_type(bytes) },
        ValueType::Thunk,
        "shallow: expected Thunk"
    );
    unsafe { handles::transform(result, bytes, 2) }
}

/// # Safety
/// `bytes` must be a handle supplied by the runtime.
#[inline]
pub unsafe extern "C" fn fixpoint_len(bytes: Reference) -> usize {
    let length = unsafe { handles::query(bytes, Query::Length) };
    assert_ne!(length, usize::MAX, "len: expected Object or Ref");
    length
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
