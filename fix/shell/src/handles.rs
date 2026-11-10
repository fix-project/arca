use crate::rt::{HANDLE_BASE, PAYLOAD_SIZE};
use core::arch::naked_asm;
pub type Reference = *mut u8;

#[derive(Clone, Copy)]
#[repr(usize)]
pub enum Query {
    Type,
    DataType,
    EncodeType,
    Comparable,
    Tag,
    Length,
    Null,
    Local,
    Value,
    LocalAddress,
    ThunkType,
    Literal,
}

pub fn address(value: Reference) -> usize {
    let address = value as usize;
    assert!((HANDLE_BASE..HANDLE_BASE + PAYLOAD_SIZE).contains(&address));
    assert!(address.is_multiple_of(32));
    address
}

pub fn bytes(value: Reference) -> [u8; 8] {
    (value as usize as u64).to_le_bytes()
}

/// # Safety
/// `value` must be a reference supplied by the shell or runtime.
pub unsafe fn query(value: Reference, operation: Query) -> usize {
    unsafe { observe(address(value), operation) }
}

/// # Safety
/// `value` must be a valid reference and adding `flags` must preserve handle validity.
pub unsafe fn transform(result: Reference, value: Reference, flags: u8) {
    unsafe { copy_handle(address(value), address(result), flags) };
}

/// # Safety
/// `cell` must be a fresh writable arena cell and `metadata` must describe `bytes`.
#[unsafe(naked)]
pub unsafe extern "C" fn create_local(_cell: usize, _address: usize, _bytes: usize, _metadata: u16) {
    naked_asm!(
        "mov [rdi], rsi",
        "mov qword ptr [rdi + 8], 0",
        "mov qword ptr [rdi + 16], 0",
        "mov [rdi + 24], rdx",
        "mov [rdi + 30], cx",
        "cmp edi, edi",
        "ret",
    );
}

/// # Safety
/// `cell` must identify a valid handle in typed storage.
pub unsafe fn query_cell(cell: usize, operation: Query) -> usize {
    unsafe { observe(cell, operation) }
}

// Raw handle bits may only reside in typed storage or registers cleared before return.
#[unsafe(naked)]
pub unsafe extern "C" fn copy_handle(_source: usize, _destination: usize, _flags: u8) {
    naked_asm!(
        "mov r10, [rdi]",
        "mov [rsi], r10",
        "mov r10, [rdi + 8]",
        "mov [rsi + 8], r10",
        "mov r10, [rdi + 16]",
        "mov [rsi + 16], r10",
        "mov r10, [rdi + 24]",
        "mov [rsi + 24], r10",
        "or byte ptr [rsi + 31], dl",
        "xor r10d, r10d",
        "cmp r10d, r10d",
        "ret",
    );
}

#[unsafe(naked)]
pub unsafe extern "C" fn same_handle_bytes(_lhs: usize, _rhs: usize) -> u32 {
    naked_asm!(
        "mov r10, [rdi]",
        "cmp r10, [rsi]",
        "jne 2f",
        "mov r10, [rdi + 8]",
        "cmp r10, [rsi + 8]",
        "jne 2f",
        "mov r10, [rdi + 16]",
        "cmp r10, [rsi + 16]",
        "jne 2f",
        "mov r10, [rdi + 24]",
        "cmp r10, [rsi + 24]",
        "jne 2f",
        "mov eax, 1",
        "jmp 3f",
        "2:",
        "xor eax, eax",
        "3:",
        "xor r10d, r10d",
        "ret",
    );
}

#[unsafe(naked)]
unsafe extern "C" fn observe(_address: usize, _operation: Query) -> usize {
    naked_asm!(
        "movzx r10d, byte ptr [rdi + 31]",
        "xor eax, eax",
        "cmp esi, {literal_query}",
        "je 15f",
        "cmp esi, {type_query}",
        "je 2f",
        "cmp esi, {data_query}",
        "je 3f",
        "cmp esi, {thunk_query}",
        "je 14f",
        "cmp esi, {encode_query}",
        "je 4f",
        "cmp esi, {comparable_query}",
        "je 5f",
        "cmp esi, {tag_query}",
        "je 6f",
        "cmp esi, {length_query}",
        "je 7f",
        "cmp esi, {local_address_query}",
        "je 13f",
        "cmp esi, {local_query}",
        "je 11f",
        "cmp esi, {value_query}",
        "je 12f",
        "cmp esi, {null_query}",
        "je 8f",
        "jmp 9f",
        "2:",
        "test r10b, 3",
        "jnz 20f",
        "test r10b, 12",
        "jnz 21f",
        "test r10b, 16",
        "setnz al",
        "jmp 10f",
        "20:",
        "mov eax, 3",
        "jmp 10f",
        "21:",
        "mov eax, 2",
        "jmp 10f",
        "3:",
        "test r10b, 15",
        "jnz 9f",
        "test r10b, 32",
        "setnz al",
        "jmp 10f",
        "4:",
        "mov eax, r10d",
        "and eax, 3",
        "jz 9f",
        "dec eax",
        "jmp 10f",
        "5:",
        "test r10b, 15",
        "jnz 10f",
        "mov eax, 1",
        "test r10b, 224",
        "jz 10f",
        "movzx eax, byte ptr [rdi + 30]",
        "shr eax, 2",
        "and eax, 1",
        "jmp 10f",
        "6:",
        "cmp r10b, 224",
        "sete al",
        "jmp 10f",
        "7:",
        "test r10b, 15",
        "jnz 9f",
        "test r10b, 224",
        "jnz 70f",
        "movzx eax, byte ptr [rdi + 30]",
        "jmp 10f",
        "70:",
        "mov rax, [rdi + 24]",
        "shl rax, 16",
        "shr rax, 16",
        "test r10b, 32",
        "jz 10f",
        "shr rax, 5",
        "jmp 10f",
        "8:",
        "test r10b, r10b",
        "jnz 10f",
        "cmp byte ptr [rdi + 30], 0",
        "sete al",
        "jmp 10f",
        "11:",
        "test r10b, 224",
        "jz 10f",
        "movzx r10d, byte ptr [rdi + 30]",
        "and r10d, 3",
        "cmp r10d, 3",
        "sete al",
        "jmp 10f",
        "13:",
        "test r10b, 224",
        "jz 10f",
        "movzx r10d, byte ptr [rdi + 30]",
        "and r10d, 3",
        "cmp r10d, 3",
        "jne 10f",
        "mov rax, [rdi]",
        "jmp 10f",
        "12:",
        "test r10b, 3",
        "jnz 10f",
        "mov eax, 1",
        "test r10b, 31",
        "jnz 10f",
        "and r10d, 160",
        "cmp r10d, 160",
        "jne 10f",
        "movzx eax, byte ptr [rdi + 30]",
        "shr eax, 3",
        "and eax, 1",
        "jmp 10f",
        "14:",
        "mov eax, r10d",
        "and eax, 12",
        "jz 9f",
        "shr eax, 2",
        "cmp eax, 2",
        "je 10f",
        "cmp eax, 3",
        "sete al",
        "movzx eax, al",
        "jmp 10f",
        "15:",
        "test r10b, r10b",
        "sete al",
        "jmp 10f",
        "9:",
        "mov rax, -1",
        "10:",
        "xor r10d, r10d",
        "cmp r10d, r10d",
        "ret",
        literal_query = const Query::Literal as usize,
        type_query = const Query::Type as usize,
        data_query = const Query::DataType as usize,
        encode_query = const Query::EncodeType as usize,
        thunk_query = const Query::ThunkType as usize,
        comparable_query = const Query::Comparable as usize,
        tag_query = const Query::Tag as usize,
        length_query = const Query::Length as usize,
        null_query = const Query::Null as usize,
        local_query = const Query::Local as usize,
        value_query = const Query::Value as usize,
        local_address_query = const Query::LocalAddress as usize,
    );
}
