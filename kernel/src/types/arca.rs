use alloc::{collections::vec_deque::VecDeque, vec::Vec};
use arcane::SyscallError;

use crate::{cpu::ExitReason, prelude::*};

use super::Value;

use crate::types::internal;

/// Layout of a compacted XSAVE area for x87 + SSE + AVX (XCR0 bits 0..=2):
///
///   [  0 ..  512)  Legacy region  – x87 state (108 B) + SSE XMM/MXCSR (416 B)
///   [512 ..  576)  XSAVE header   – 64-byte XSTATE_BV / XCOMP_BV bitmap
///   [576 ..  832)  AVX component  – 16 × 16 B YMM_Hi128 registers
///
/// Total: 832 bytes.  Rounded up to 1024 for headroom.
/// Confirmed sufficient by `xstate::assert_buffer_adequate()` at runtime.
const XSAVE_AREA_BYTES: usize = 1024;

#[repr(C, align(64))]
#[derive(Clone, Eq, PartialEq)]
pub struct XsaveArea([u8; XSAVE_AREA_BYTES]);

impl XsaveArea {
    fn new_boxed() -> Box<Self> {
        // Rust's allocator contract requires respecting the type's alignment,
        // but we assert here to catch any non-conformant allocators early.
        let b = Box::new(Self([0u8; XSAVE_AREA_BYTES]));
        debug_assert_eq!(
            b.0.as_ptr() as usize % 64,
            0,
            "XsaveArea heap allocation is not 64-byte aligned; \
             global allocator does not honour over-alignment"
        );
        b
    }

    fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }
    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }
}

impl core::fmt::Debug for XsaveArea {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("XsaveArea")
            .field(&format_args!("[...; {}]", XSAVE_AREA_BYTES))
            .finish()
    }
}

#[cfg(target_arch = "x86_64")]
mod xstate {
    use core::arch::x86_64::{__cpuid, __cpuid_count};

    // ── Feature detection ────────────────────────────────────────────────────

    /// Returns `true` if the OS has set `CR4.OSXSAVE`, making `XGETBV`/`XSAVE`
    // safe to execute.  Must be verified before any other function in this module.
    #[inline(always)]
    pub fn osxsave_enabled() -> bool {
        // CPUID.01H:ECX bit 27 = OSXSAVE
        let r = __cpuid(0x1);
        (r.ecx >> 27) & 1 == 1
    }

    /// Returns `true` if the OS has enabled AVX state saving in XCR0 (bit 2).
    ///
    /// Gate any AVX-specific code on this rather than assuming AVX is always
    /// present just because the CPU supports it.
    // #[inline(always)]
    // pub fn avx_enabled() -> bool {
    //     xcr0() & 0x4 != 0
    // }

    /// Returns `true` if XSAVEOPT is available (CPUID.(EAX=0Dh,ECX=1h).EAX bit 0).
    ///
    /// XSAVEOPT skips saving components whose state has not changed since the
    /// last XRSTOR, making context-switch saves cheaper when xstate is clean.
    #[inline(always)]
    pub fn xsaveopt_available() -> bool {
        let r = __cpuid_count(0xD, 1);
        r.eax & 1 == 1
    }

    /// Return XCR0 bits (OS-enabled xfeatures).
    ///
    /// # Safety
    /// OSXSAVE must be set; otherwise `XGETBV` raises `#UD`.
    #[inline(always)]
    pub fn xcr0() -> u64 {
        unsafe { core::arch::x86_64::_xgetbv(0) }
    }

    /// Save mask: x87 (bit 0) + SSE (bit 1) + AVX if OS-enabled (bit 2).
    ///
    /// Bits beyond 2 (MPX, AVX-512, …) are intentionally excluded; they require
    /// a larger XSAVE area than this module manages.
    #[inline(always)]
    pub fn save_mask() -> u64 {
        xcr0() & 0x7
    }

    // ── Buffer-size validation ───────────────────────────────────────────────

    /// Compute required XSAVE buffer bytes for the given mask.
    ///
    /// For mask = xcr0 & 0x7 (x87 + SSE + AVX only), bits 0 and 1 always
    /// fit in the 576-byte legacy+header region.  Only bit 2 (AVX / YMM_Hi128)
    /// adds beyond that, so we only need to query sub-leaf 2.
    ///
    /// CPUID.(0xD, n):
    ///   EAX = size   of component n in bytes
    ///   EBX = offset of component n from start of XSAVE area
    pub fn required_bytes_for_mask(mask: u64) -> usize {
        // Legacy x87+SSE region (512 B) + XSAVE header (64 B), always required.
        let mut needed: usize = 512 + 64;

        // Only check components that are actually in our mask.
        // For mask & 0x7 this is at most bit 2 (AVX).
        for bit in 2u32..64 {
            if mask & (1u64 << bit) == 0 {
                continue;
            }
            let r = __cpuid_count(0xD, bit);
            let size = r.eax as usize; // ← was r.ebx, wrong
            let offset = r.ebx as usize; // ← was r.ecx, wrong
            needed = needed.max(offset + size);
        }

        needed
    }

    /// Assert that `buf_bytes` is sufficient for the current `save_mask()` and
    /// that OSXSAVE is set.  Call once during `Arca::new()`.
    pub fn assert_buffer_adequate(buf_bytes: usize) {
        assert!(
            osxsave_enabled(),
            "OSXSAVE is not set in CR4; XSAVE instructions will #UD"
        );

        let mask = save_mask();
        let needed = required_bytes_for_mask(mask);
        assert!(
            needed <= buf_bytes,
            "XSAVE buffer too small: mask {:#x} needs {} bytes, have {}",
            mask,
            needed,
            buf_bytes,
        );
    }

    // ── Save / restore ───────────────────────────────────────────────────────

    /// Save xstate for `mask` into `dst`.
    ///
    /// Uses XSAVEOPT when available (skips unmodified components), falling back
    /// to plain XSAVE otherwise.
    ///
    /// # Safety
    /// - `dst` must be 64-byte aligned and at least `required_bytes_for_mask(mask)` bytes.
    /// - The buffer must already contain a valid prior XSAVE image (XSTATE_BV set).
    ///   Use `snapshot_current_xstate` to initialise it.
    /// - OSXSAVE must be set.
    #[inline(always)]
    pub unsafe fn save_xstate(dst: *mut u8, mask: u64) {
        if xsaveopt_available() {
            core::arch::x86_64::_xsaveopt(dst, mask);
        } else {
            core::arch::x86_64::_xsave(dst, mask);
        }
    }

    /// Restore xstate for `mask` from `src`.
    ///
    /// # Safety
    /// - `src` must be 64-byte aligned and contain a valid XSAVE image for `mask`.
    /// - OSXSAVE must be set.
    #[inline(always)]
    pub unsafe fn restore_xstate(src: *const u8, mask: u64) {
        core::arch::x86_64::_xrstor(src, mask);
    }

    /// Snapshot the *current* CPU xstate into `dst` using plain XSAVE.
    ///
    /// Always uses plain XSAVE (never XSAVEOPT) so all component fields are
    /// unconditionally written and XSTATE_BV is fully populated, making the
    /// buffer safe for a subsequent `save_xstate` (XSAVEOPT) / `restore_xstate`.
    ///
    /// # Safety
    /// Same alignment/size requirements as `save_xstate`.
    #[inline(always)]
    pub unsafe fn snapshot_current_xstate(dst: *mut u8, mask: u64) {
        core::arch::x86_64::_xsave(dst, mask);
    }

    /// Zero the AVX (YMM_Hi128) region in an XSAVE buffer.
    ///
    /// Layout: [576 .. 832) = 16 × 16 B YMM_Hi128. Leaves x87, SSE, and header intact
    /// so XRSTOR still validates. Use when initialising a fresh Arca so it does not
    /// inherit the previous context's SIMD state from `snapshot_current_xstate`.
    ///
    /// # Safety
    /// `dst` must be a valid XSAVE buffer with at least 832 bytes, 64-byte aligned.
    #[inline(always)]
    pub unsafe fn zero_avx_region(dst: *mut u8) {
        core::ptr::write_bytes(dst.add(576), 0, 256);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arca {
    page_table: Table,
    register_file: Box<RegisterFile>,
    descriptors: Descriptors,
    fsbase: u64,
    xsave_area: Box<XsaveArea>,
    // rlimit: Resources,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resources {
    pub memory: usize,
}

impl Arca {
    pub fn new() -> Arca {
        let page_table = Table::from_inner(internal::Table::default());
        let register_file = RegisterFile::new().into();
        let descriptors = Descriptors::new();
        let mut xsave_area = XsaveArea::new_boxed();
        // let rlimit = Resources { memory: 1 << 21 };

        #[cfg(target_arch = "x86_64")]
        {
            xstate::assert_buffer_adequate(XSAVE_AREA_BYTES);

            let mask = xstate::save_mask();
            // Initialise with a valid image rather than all-zeros.
            // An all-zeros buffer fails XRSTOR validation under KVM because
            // XSTATE_BV and XCOMP_BV are not correctly set.
            unsafe { xstate::snapshot_current_xstate(xsave_area.as_mut_ptr(), mask) };
            // Zero YMM so a fresh Arca does not inherit the previous context's SIMD state.
            if mask & 0x4 != 0 {
                unsafe { xstate::zero_avx_region(xsave_area.as_mut_ptr()) };
            }
        }

        Arca {
            page_table,
            register_file,
            descriptors,
            fsbase: 0,
            xsave_area,
            // rlimit,
        }
    }

    pub fn new_with(
        register_file: impl Into<Box<RegisterFile>>,
        page_table: Table,
        descriptors: Tuple,
        _rlimit: Tuple,
    ) -> Arca {
        let descriptors = Vec::from(descriptors.into_inner().into_inner()).into();
        let mut xsave_area = XsaveArea::new_boxed();

        #[cfg(target_arch = "x86_64")]
        {
            xstate::assert_buffer_adequate(XSAVE_AREA_BYTES);
            let mask = xstate::save_mask();
            unsafe { xstate::snapshot_current_xstate(xsave_area.as_mut_ptr(), mask) };
            if mask & 0x4 != 0 {
                unsafe { xstate::zero_avx_region(xsave_area.as_mut_ptr()) };
            }
        }

        // let mem_limit = Word::try_from(rlimit.get(0).clone()).unwrap().read() as usize;
        // let rlimit = Resources { memory: mem_limit };

        Arca {
            page_table,
            register_file: register_file.into(),
            descriptors,
            fsbase: 0,
            xsave_area,
            // rlimit,
        }
    }

    pub fn load(self, cpu: &mut Cpu) -> LoadedArca<'_> {
        // let memory = ValueRef::Table(&self.page_table).byte_size()
        //     + self
        //         .descriptors
        //         .iter()
        //         .map(|x| x.byte_size())
        //         .reduce(|x, y| x + y)
        //         .unwrap();

        cpu.activate_address_space(self.page_table.into_inner());
        unsafe {
            core::arch::asm! {
                "wrfsbase {base}",
                base = in(reg) self.fsbase,
                options(nostack, preserves_flags)
            };

            #[cfg(target_arch = "x86_64")]
            {
                let mask = xstate::save_mask();
                // Restore x87, SSE, and AVX (if bit 2 is set in mask) onto the CPU.
                xstate::restore_xstate(self.xsave_area.as_ptr(), mask);
            }
        }

        // let rusage = Resources { memory };
        // assert!(rusage.memory <= self.rlimit.memory);
        LoadedArca {
            register_file: self.register_file,
            descriptors: self.descriptors,
            cpu,
            xsave_area: self.xsave_area,
            // rlimit: self.rlimit,
            // rusage,
        }
    }

    pub fn registers(&self) -> &RegisterFile {
        &self.register_file
    }

    pub fn registers_mut(&mut self) -> &mut RegisterFile {
        &mut self.register_file
    }

    pub fn mappings(&self) -> &Table {
        &self.page_table
    }

    pub fn mappings_mut(&mut self) -> &mut Table {
        &mut self.page_table
    }

    pub fn descriptors(&self) -> &Descriptors {
        &self.descriptors
    }

    pub fn descriptors_mut(&mut self) -> &mut Descriptors {
        &mut self.descriptors
    }

    pub fn read(self) -> (RegisterFile, Table, Tuple) {
        (
            *self.register_file,
            self.page_table,
            Tuple::from_inner(internal::Tuple::new(Vec::from(self.descriptors))),
        )
    }

    // pub fn rlimit(&self) -> &Resources {
    //     &self.rlimit
    // }

    // pub fn rlimit_mut(&mut self) -> &mut Resources {
    //     &mut self.rlimit
    // }
}

impl Default for Arca {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
pub(crate) fn required_bytes_for_avx_mask() -> usize {
    xstate::required_bytes_for_mask(0x7)
}

#[cfg(test)]
pub(crate) fn xsave_area_alignment() -> usize {
    core::mem::align_of::<XsaveArea>()
}

#[derive(Debug)]
pub struct LoadedArca<'a> {
    register_file: Box<RegisterFile>,
    descriptors: Descriptors,
    cpu: &'a mut Cpu,
    xsave_area: Box<XsaveArea>,
    // rlimit: Resources,
    // rusage: Resources,
}

impl<'a> LoadedArca<'a> {
    pub fn run(&mut self) -> ExitReason {
        unsafe { self.cpu.run(&mut self.register_file) }
    }

    pub fn registers(&self) -> &RegisterFile {
        &self.register_file
    }

    pub fn registers_mut(&mut self) -> &mut RegisterFile {
        &mut self.register_file
    }

    pub fn descriptors(&self) -> &Descriptors {
        &self.descriptors
    }

    pub fn descriptors_mut(&'_ mut self) -> DescriptorsProxy<'_, 'a> {
        DescriptorsProxy { arca: self }
    }

    // pub fn rlimit(&self) -> &Resources {
    //     &self.rlimit
    // }

    // pub fn rlimit_mut(&mut self) -> &mut Resources {
    //     &mut self.rlimit
    // }

    // pub fn rusage(&self) -> &Resources {
    //     &self.rusage
    // }

    // pub fn rusage_mut(&mut self) -> &mut Resources {
    //     &mut self.rusage
    // }

    pub fn unload(self) -> Arca {
        self.unload_with_cpu().0
    }

    pub fn take(&mut self) -> Arca {
        let mut original = Default::default();
        self.swap(&mut original);
        original
    }

    pub fn unload_with_cpu(self) -> (Arca, &'a mut Cpu) {
        // add SIMD support here?
        let page_table = Table::from_inner(self.cpu.deactivate_address_space());
        let mut fsbase: u64;
        let mut xsave_area = self.xsave_area;

        unsafe {
            core::arch::asm! {
                "rdfsbase {base}",
                base = out(reg) fsbase,
                options(nostack, preserves_flags)
            };

            #[cfg(target_arch = "x86_64")]
            {
                let mask = xstate::save_mask();
                // XSAVEOPT skips components unmodified since the last XRSTOR,
                // reducing save cost on clean context switches.
                xstate::save_xstate(xsave_area.as_mut_ptr(), mask);
            }
        }

        (
            Arca {
                register_file: self.register_file,
                descriptors: self.descriptors,
                page_table,
                fsbase,
                xsave_area,
                // rlimit: self.rlimit,
            },
            self.cpu,
        )
    }

    pub fn swap(&mut self, other: &mut Arca) {
        core::mem::swap(&mut self.register_file, &mut other.register_file);
        core::mem::swap(&mut self.descriptors, &mut other.descriptors);
        // core::mem::swap(&mut self.rlimit, &mut other.rlimit);
        let mut fsbase: u64;
        unsafe {
            core::arch::asm!(
                "rdfsbase {old}; wrfsbase {new}",
                old = out(reg) fsbase,
                new = in(reg) other.fsbase,
                options(nostack, preserves_flags)
            );

            #[cfg(target_arch = "x86_64")]
            {
                let mask = xstate::save_mask();

                // Step 1 – save the currently-live (self) xstate into self's buffer.
                //           The buffer was initialised by snapshot_current_xstate and
                //           kept valid by prior save/restore pairs, so XSAVEOPT is safe.
                xstate::save_xstate(self.xsave_area.as_mut_ptr(), mask);

                // Step 2 – load other's saved xstate onto the CPU.
                xstate::restore_xstate(other.xsave_area.as_ptr(), mask);

                // At this point the buffers are intentionally asymmetric:
                //   self.xsave_area  holds self's just-captured state
                //   other.xsave_area holds other's prior saved state
                // The mem::swap below corrects ownership so after it:
                //   self.xsave_area  owns other's saved state (now live on CPU)
                //   other.xsave_area owns self's captured state
            }
        }
        other.fsbase = fsbase;
        core::mem::swap(&mut self.xsave_area, &mut other.xsave_area);

        self.cpu.swap_address_space(other.page_table.inner_mut());
    }

    pub fn cpu(&'_ mut self) -> CpuProxy<'_, 'a> {
        CpuProxy { arca: self }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DescriptorError {
    AttemptToMutateNull,
    OutOfBounds,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Descriptors {
    descriptors: Vec<Value>,
}

pub type Result<T> = core::result::Result<T, DescriptorError>;

impl Descriptors {
    pub fn new() -> Descriptors {
        Descriptors {
            descriptors: vec![Value::default()],
        }
    }

    pub fn get(&self, index: usize) -> Result<&Value> {
        self.descriptors
            .get(index)
            .ok_or(DescriptorError::OutOfBounds)
    }

    pub fn get_mut(&mut self, index: usize) -> Result<&mut Value> {
        if index == 0 {
            return Err(DescriptorError::AttemptToMutateNull);
        }
        self.descriptors
            .get_mut(index)
            .ok_or(DescriptorError::OutOfBounds)
    }

    pub fn take(&mut self, index: usize) -> Result<Value> {
        if index == 0 {
            return Ok(Value::default());
        }
        Ok(core::mem::take(
            self.descriptors
                .get_mut(index)
                .ok_or(DescriptorError::OutOfBounds)?,
        ))
    }

    pub fn insert(&mut self, value: Value) -> usize {
        if value.datatype() == DataType::Null {
            return 0;
        }
        for (i, x) in self.descriptors.iter_mut().enumerate().skip(1) {
            if x.datatype() == DataType::Null {
                *x = value;
                return i;
            }
        }
        let i = self.descriptors.len();
        self.descriptors.push(value);
        i
    }

    pub fn iter(&'_ self) -> Iter<'_> {
        Iter { i: 0, d: self }
    }
}

pub struct Iter<'a> {
    i: usize,
    d: &'a Descriptors,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Value;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.d.descriptors.len() {
            return None;
        }
        self.i += 1;
        Some(self.d.get(self.i - 1).unwrap())
    }
}

impl Default for Descriptors {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Vec<Value>> for Descriptors {
    fn from(mut value: Vec<Value>) -> Self {
        if value.is_empty() {
            value.push(Default::default());
        } else {
            value[0] = Default::default();
        }
        Descriptors { descriptors: value }
    }
}

impl From<Descriptors> for Vec<Value> {
    fn from(value: Descriptors) -> Self {
        value.descriptors
    }
}

impl From<Arca> for super::Function {
    fn from(value: Arca) -> super::Function {
        super::Function::from_inner(super::internal::Function::arcane_with_args(
            value,
            VecDeque::default(),
        ))
    }
}

pub struct DescriptorsProxy<'a, 'cpu> {
    arca: &'a mut LoadedArca<'cpu>,
}

impl<'a> DescriptorsProxy<'a, '_> {
    pub fn insert(self, value: Value) -> core::result::Result<usize, SyscallError> {
        // let size = value.byte_size();
        // if self.arca.rusage().memory + size > self.arca.rlimit().memory {
        //     return Err(SyscallError::OutOfMemory);
        // }
        // self.arca.rusage_mut().memory += size;
        Ok(self.arca.descriptors.insert(value))
    }

    pub fn take(self, index: usize) -> core::result::Result<Value, SyscallError> {
        let value = self.arca.descriptors.take(index)?;
        // let size = value.byte_size();
        // self.arca.rusage_mut().memory -= size;
        Ok(value)
    }

    pub fn get(self, index: usize) -> core::result::Result<&'a Value, DescriptorError> {
        self.arca.descriptors.get(index)
    }

    pub fn get_mut(self, index: usize) -> core::result::Result<&'a mut Value, DescriptorError> {
        self.arca.descriptors.get_mut(index)
    }
}

pub struct CpuProxy<'a, 'cpu> {
    arca: &'a mut LoadedArca<'cpu>,
}

impl<'a> CpuProxy<'a, '_> {
    pub fn map(
        &mut self,
        address: usize,
        entry: Entry,
    ) -> core::result::Result<Entry, SyscallError> {
        // let limit = self.arca.rlimit().memory;
        // let new_size = entry.byte_size();
        // let old = self.arca.cpu.map(address, Entry::Null(new_size))?;
        // let old_size = old.byte_size();
        // let usage = &mut self.arca.rusage_mut().memory;
        // *usage -= old_size;
        // if *usage + new_size > limit {
        //     self.arca.cpu.map(address, old)?;
        //     self.arca.rusage_mut().memory += old_size;
        //     return Err(SyscallError::OutOfMemory);
        // }
        let old = self.arca.cpu.map(address, entry)?;
        // self.arca.rusage_mut().memory += new_size;
        Ok(old)
    }
}
