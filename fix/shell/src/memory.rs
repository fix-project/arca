use fixhandle::accounting;

static mut FRAME: usize = 0;
static mut ENCODED: usize = 0;
static mut MAPPED: [usize; 127] = [0; 127];
static mut DESCRIPTORS: usize = 0;

pub fn initialize() {
    unsafe extern "C" {
        static _stext: u8;
        static __image_text_end: u8;
        static __image_data_start: u8;
        static __image_data_end: u8;
        static __stack_bottom: u8;
        static __stack_top: u8;
    }
    unsafe {
        FRAME = accounting::INVOCATION;
        ENCODED = accounting::encoded_metadata(accounting::INVOCATION)
            .saturating_add(accounting::table_encoding());
        MAPPED[crate::rt::HANDLE_BASE / crate::rt::SLOT_SIZE] = accounting::PAGE;
    }
    for (start, end) in [
        (
            (&raw const _stext) as usize,
            (&raw const __image_text_end) as usize,
        ),
        (
            (&raw const __image_data_start) as usize,
            (&raw const __image_data_end) as usize,
        ),
        (
            (&raw const __stack_bottom) as usize,
            (&raw const __stack_top) as usize,
        ),
    ] {
        let address = start / accounting::PAGE * accounting::PAGE;
        let bytes = end
            .next_multiple_of(accounting::PAGE)
            .saturating_sub(address);
        add_frame(
            accounting::mapping(bytes),
            accounting::encoded_mapping(address, bytes),
        );
    }
}

pub fn charge(bytes: usize) {
    if bytes != 0 {
        let bytes = i64::try_from(bytes).unwrap_or(i64::MAX);
        assert_eq!(unsafe { arcane::arca_memory_adjust(bytes) }, 0);
    }
}

pub fn release(bytes: usize) {
    if bytes != 0 {
        let bytes = i64::try_from(bytes).expect("invalid memory release");
        assert_eq!(unsafe { arcane::arca_memory_adjust(-bytes) }, 0);
    }
}

pub fn add_frame(bytes: usize, encoded: usize) {
    charge(bytes);
    unsafe {
        FRAME = FRAME
            .checked_add(bytes)
            .expect("memory accounting overflow");
        ENCODED = ENCODED
            .checked_add(encoded)
            .expect("checkpoint accounting overflow");
    }
}

pub fn remove_frame(bytes: usize, encoded: usize) {
    unsafe {
        FRAME = FRAME.checked_sub(bytes).expect("unbalanced frame release");
        ENCODED = ENCODED
            .checked_sub(encoded)
            .expect("unbalanced checkpoint release");
    }
    release(bytes);
}

pub fn descriptors(count: usize) {
    let bytes = count * accounting::DESCRIPTOR;
    let previous = unsafe { DESCRIPTORS };
    let next = previous
        .checked_add(bytes)
        .expect("descriptor accounting overflow");
    add_frame(
        bytes,
        accounting::encoded_metadata(next).saturating_sub(accounting::encoded_metadata(previous)),
    );
    unsafe {
        DESCRIPTORS = next;
    }
}

pub fn snapshot() -> usize {
    unsafe {
        FRAME
            .saturating_add(ENCODED)
            .saturating_add(accounting::BUDGET)
    }
}

pub fn become_scratch(_address: usize, bytes: usize) {
    unsafe {
        FRAME = FRAME.saturating_add(bytes.next_multiple_of(accounting::PAGE));
        ENCODED = ENCODED.saturating_add(accounting::encoded_leaves(bytes));
    }
}

pub fn mapping(address: usize, bytes: usize, writable: bool) {
    let index = address / crate::rt::SLOT_SIZE;
    let base = index * crate::rt::SLOT_SIZE;
    let end = (address - base)
        .saturating_add(bytes)
        .next_multiple_of(accounting::PAGE);
    let previous = unsafe { MAPPED[index] };
    let high = previous.max(end);
    add_frame(
        (high - previous).saturating_mul(4),
        accounting::encoded_nodes(base, high)
            .saturating_sub(accounting::encoded_nodes(base, previous)),
    );
    unsafe {
        MAPPED[index] = high;
    }
    let backing = bytes.next_multiple_of(accounting::PAGE);
    if writable {
        add_frame(backing, accounting::encoded_leaves(bytes));
    } else {
        charge(backing);
    }
}

pub fn unmap(address: usize, bytes: usize) {
    let index = address / crate::rt::SLOT_SIZE;
    let attached = index < crate::rt::NUM_MEMORIES && unsafe { crate::rt::MEMORY_ATTACHED[index] };
    let backing = bytes.next_multiple_of(accounting::PAGE);
    if attached {
        release(backing);
    } else {
        remove_frame(backing, accounting::encoded_leaves(bytes));
    }
}

pub struct Working {
    bytes: usize,
    encoded: usize,
}

impl Working {
    pub fn new(bytes: usize) -> Self {
        let encoded = accounting::encoded_metadata(bytes);
        add_frame(bytes, encoded);
        Self { bytes, encoded }
    }
}

impl Drop for Working {
    fn drop(&mut self) {
        remove_frame(self.bytes, self.encoded);
    }
}
