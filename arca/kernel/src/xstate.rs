use core::{
    arch::asm,
    sync::atomic::{AtomicU32, Ordering},
};

const STATE_MASK: u32 = 0x7;
const XSAVE_SIZE: usize = 832;
static MXCSR_MASK: AtomicU32 = AtomicU32::new(0xffff);

/// Standard XSAVE format: x87/SSE (512 bytes), header (64), and YMM upper halves (256).
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(C, align(64))]
pub struct XState {
    bytes: [u8; XSAVE_SIZE],
}

impl Default for XState {
    fn default() -> Self {
        let mut bytes = [0; XSAVE_SIZE];
        // XRSTOR initializes absent components, but always loads MXCSR from memory.
        bytes[24..28].copy_from_slice(&0x1f80u32.to_le_bytes());
        Self { bytes }
    }
}

impl XState {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let bytes: [u8; XSAVE_SIZE] = bytes.try_into().ok()?;
        let components = u64::from_le_bytes(bytes[512..520].try_into().unwrap());
        let mxcsr = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
        // Untrusted function definitions must not cause XRSTOR to fault in the kernel.
        if components & !u64::from(STATE_MASK) != 0
            || bytes[520..576].iter().any(|&x| x != 0)
            || mxcsr & !MXCSR_MASK.load(Ordering::Relaxed) != 0
        {
            return None;
        }
        Some(Self { bytes })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn save(&mut self) {
        unsafe {
            asm!(
                "xsave64 [{area}]",
                area = in(reg) self.bytes.as_mut_ptr(),
                in("eax") STATE_MASK,
                in("edx") 0u32,
                options(nostack, preserves_flags),
            );
        }
    }

    pub fn restore(&self) {
        // Kernel code uses the target's soft-float ABI and leaves SIMD registers alone.
        unsafe {
            asm!(
                "xrstor64 [{area}]",
                area = in(reg) self.bytes.as_ptr(),
                in("eax") STATE_MASK,
                in("edx") 0u32,
                options(nostack, preserves_flags),
            );
        }
    }
}

pub fn init() {
    use core::arch::x86_64::{__cpuid, __cpuid_count};

    unsafe {
        let features = __cpuid(1).ecx;
        assert!(
            features & (1 << 26 | 1 << 27 | 1 << 28) == (1 << 26 | 1 << 27 | 1 << 28),
            "Arca requires XSAVE, OSXSAVE, and AVX"
        );
        assert!(
            __cpuid_count(7, 0).ebx & (1 << 5) != 0,
            "Arca requires AVX2"
        );
        assert!(__cpuid_count(0xd, 0).eax & STATE_MASK == STATE_MASK);
        let avx = __cpuid_count(0xd, 2);
        assert!(
            avx.eax == 256 && avx.ebx == 576,
            "unsupported AVX state layout"
        );
        asm!(
            "xsetbv",
            in("ecx") 0u32,
            in("eax") STATE_MASK,
            in("edx") 0u32,
            options(nostack, preserves_flags),
        );
    }
    let mut state = XState::default();
    state.save();
    let mask = u32::from_le_bytes(state.bytes[28..32].try_into().unwrap());
    MXCSR_MASK.fetch_and(if mask == 0 { 0xffbf } else { mask }, Ordering::Relaxed);
}

const _: () = {
    assert!(core::mem::size_of::<XState>() == XSAVE_SIZE);
    assert!(core::mem::align_of::<XState>() == 64);
};
