#![cfg_attr(target_arch = "wasm32", no_std, feature(asm_experimental_arch))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
pub use fixhandle::*;
pub use macros::fix_entrypoint;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Producer {
    // memory_index, length
    CreateBlob = 2,
}

fn encode_args(producer: Producer, index: u32, entry: u32) -> RawName {
    debug_assert!(index < 256);
    let mut bytes = [0; 32];
    // 4 bytes for entry/length argument
    bytes[24..28].copy_from_slice(&entry.to_le_bytes());
    // One byte for producer type
    bytes[28] = producer as u8;
    // One byte for table/memory index
    bytes[29] = index as u8;
    RawName::forge(bytes)
}

#[repr(C, align(8))]
pub struct RustHandle([u8; 32]);

impl RustHandle {
    fn new(handle: Handle) -> Self {
        Self(handle.pack())
    }
}

core::arch::global_asm!(
    r#"
    .globl memory_1_read
memory_1_read:
    .functype memory_1_read (i32, i32) -> ()
        local.get 0
        i32.const 0
        local.get 1
        memory.copy 0, 1
    end_function

    .globl memory_1_write
memory_1_write:
    .functype memory_1_write (i32, i32) -> ()
        i32.const 0
        local.get 0
        local.get 1
        memory.copy 1, 0
    end_function
    "#
);

unsafe extern "C" {
    // Copies length bytes from memory 1 to destination in program memory 0
    pub fn memory_1_read(destination: u32, length: usize);

    // Copies length bytes from source in program memory 0 to memory 1
    pub fn memory_1_write(source: u32, length: usize);

    pub fn attach_blob(memory_index: u32, handle: *const RustHandle);
    pub fn len(handle: *const RustHandle) -> usize;
}

pub fn create_blob(memory_index: u32, length: usize) -> RustHandle {
    RustHandle::new(Handle::Object(Object::Blob(Blob::Blob(unsafe {
        BlobName::new(encode_args(
            Producer::CreateBlob,
            memory_index,
            length as u32,
        ))
    }))))
}
