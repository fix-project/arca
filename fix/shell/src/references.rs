use crate::handles::Reference;
use crate::rt::{self, HANDLE_BASE, PAYLOAD_SIZE, wasm_rt_ref_stack, wasm_rt_ref_value};

pub const BASE: usize = HANDLE_BASE + PAYLOAD_SIZE / 2;

unsafe extern "C" {
    pub fn wasm_rt_ref_reserve(count: usize) -> Reference;
}

pub unsafe fn init() {
    unsafe {
        rt::wasm_rt_refs = wasm_rt_ref_stack {
            base: BASE as Reference,
            top: BASE as Reference,
            limit: (HANDLE_BASE + PAYLOAD_SIZE) as Reference,
            mapped: BASE as Reference,
        };
    }
}

pub fn range() -> (usize, usize) {
    let top = unsafe { rt::wasm_rt_refs.top } as usize;
    (BASE, (top - BASE) / fixhandle::HANDLE_SIZE)
}

pub fn mapped() -> usize {
    unsafe { rt::wasm_rt_refs.mapped as usize - BASE }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wasm_rt_ref_grow(_: *mut wasm_rt_ref_value, end: *mut wasm_rt_ref_value) {
    let needed = (end as usize).next_multiple_of(4096);
    unsafe {
        let mapped = rt::wasm_rt_refs.mapped as usize;
        rt::map_region(
            mapped as *mut u8,
            needed - mapped,
            arcane::__MODE_read_write,
        )
        .expect("reference stack allocation failed");
        rt::wasm_rt_refs.mapped = needed as Reference;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wasm_rt_ref_overflow() -> ! {
    panic!("reference stack exhausted")
}
