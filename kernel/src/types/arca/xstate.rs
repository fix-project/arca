use crate::prelude::*;
use bitfield_struct::bitfield;

use core::simd::u8x16;

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
#[repr(packed, C)]
pub struct XSaveArea {
    legacy: XSaveLegacy,
    header: XSaveHeader,
    extended: XSaveExtended,
}

impl XSaveArea {
    pub fn as_ptr(&self) -> *const u8 {
        &raw const *self as *const u8
    }

    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        &raw mut *self as *mut u8
    }

    pub unsafe fn as_slice(&self) -> &[u8] {
        core::slice::from_raw_parts(self.as_ptr(), core::mem::size_of::<Self>())
    }

    pub unsafe fn as_mut_slice(&mut self) -> &mut [u8] {
        core::slice::from_raw_parts_mut(self.as_mut_ptr(), core::mem::size_of::<Self>())
    }
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
#[repr(packed, C)]
pub struct XSaveLegacy {
    header: XSaveLegacyHeader,
    mm: [u8x16; 8],
    xmm: [u8x16; 16],
    _unused: [u64; 12],
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct XSaveLegacyHeader {
    fcw: u16,
    fsw: u16,
    ftw: u8,
    _0: u8,
    fop: u16,
    fip: u64,
    fdp: u64,
    mxcsr: u32,
    mxcsr_mask: u32,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
#[repr(C, packed)]
pub struct XSaveHeader {
    xstate_bv: StateComponent,
    xcomp_bv: StateComponent,
    reserved: [u64; 6],
}

#[bitfield(u64)]
#[derive(Eq, PartialEq)]
pub struct StateComponent {
    x87: bool,
    sse: bool,
    avx: bool,
    mpx_bndregs: bool,
    mpx_bndstatus: bool,
    avx512_opmask: bool,
    avx512_zmm_hi256: bool,
    avx512_hi16_zmm: bool,
    pt: bool,
    pkru: bool,
    pasid: bool,
    cet_u: bool,
    cet_s: bool,
    hdc: bool,
    uintr: bool,
    lbr: bool,
    hwp: bool,
    amx_tilecfg: bool,
    amx_tiledata: bool,
    #[bits(45)]
    _unused: u64,
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
#[repr(C, packed)]
pub struct XSaveExtended {
    ymm_hi: [u8x16; 16],
}

const {
    assert!(core::mem::size_of::<XSaveLegacyHeader>() == 32);
    assert!(core::mem::size_of::<XSaveLegacy>() == 512);
    assert!(core::mem::size_of::<XSaveHeader>() == 64);
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct XSaveData {
    pub bytes: [u8; core::mem::size_of::<XSaveArea>()],
}

impl Default for XSaveData {
    fn default() -> Self {
        XSaveData { bytes: [0; _] }
    }
}

impl From<XSaveArea> for XSaveData {
    fn from(value: XSaveArea) -> Self {
        unsafe {
            XSaveData {
                #[allow(clippy::missing_transmute_annotations)]
                bytes: core::mem::transmute(value),
            }
        }
    }
}

impl From<XSaveData> for XSaveArea {
    fn from(value: XSaveData) -> Self {
        #[allow(clippy::missing_transmute_annotations)]
        unsafe {
            core::mem::transmute(value.bytes)
        }
    }
}

impl From<XSaveData> for Box<XSaveArea> {
    fn from(value: XSaveData) -> Self {
        Box::new(value.into())
    }
}
