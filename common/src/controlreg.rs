#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlReg0;

#[allow(unused)]
impl ControlReg0 {
    /// Protected-mode Enable
    pub const PE: u64 = 1 << 0;
    /// Montior co-Processor
    pub const MP: u64 = 1 << 1;
    /// x87 Emulation
    pub const EM: u64 = 1 << 2;
    /// Task Switched
    pub const TS: u64 = 1 << 3;
    /// Extension Type
    pub const ET: u64 = 1 << 4;
    /// Numeric Error
    pub const NE: u64 = 1 << 5;
    /// Write Protect
    pub const WP: u64 = 1 << 16;
    /// Alignment Mask
    pub const AM: u64 = 1 << 18;
    /// Not Write Through
    pub const NW: u64 = 1 << 29;
    /// Cache Disable
    pub const CD: u64 = 1 << 30;
    /// Paging
    pub const PG: u64 = 1 << 31;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlReg4;

#[allow(unused)]
impl ControlReg4 {
    /// Virtual 8086 Mode Extensions
    pub const VME: u64 = 1 << 0;
    /// Protected-mode Virtual Interrupts
    pub const PVI: u64 = 1 << 1;
    /// Time Stamp Disable
    pub const TSD: u64 = 1 << 2;
    /// Debugging Extensions
    pub const DE: u64 = 1 << 3;
    /// Page Size Extension
    pub const PSE: u64 = 1 << 4;
    /// Physical Address Extension
    pub const PAE: u64 = 1 << 5;
    /// Machine Check Exception
    pub const MCE: u64 = 1 << 6;
    /// Page Global Enabled
    pub const PGE: u64 = 1 << 7;
    /// Performance-monitoring Counter Enable
    pub const PCE: u64 = 1 << 8;
    /// Operating System support for FXSAVE and FXRSTOR
    pub const OSFXSR: u64 = 1 << 9;
    /// Operating System support for Unmasked SIMD Floating-Point Exceptions
    pub const OSXMMEXCPT: u64 = 1 << 10;
    /// User-Mode Instruction Prevention
    pub const UMIP: u64 = 1 << 11;
    /// 57-Bit Linear Addresses
    pub const LA57: u64 = 1 << 12;
    /// Virtual Machine Extensions Enable
    pub const VMXE: u64 = 1 << 13;
    /// Safer Mode Extensions Enable
    pub const SMXE: u64 = 1 << 14;
    /// FSGSBASE Enable
    pub const FSGSBASE: u64 = 1 << 16;
    /// PCID Enable
    pub const PCIDE: u64 = 1 << 17;
    /// Operating System support for XSAVE
    pub const OSXSAVE: u64 = 1 << 18;
    /// Key Locker Enable
    pub const KL: u64 = 1 << 19;
    /// Supervisor Mode Execution Protection Enable
    pub const SMEP: u64 = 1 << 20;
    /// Supervisor Mode Access Prevention Enable
    pub const SMAP: u64 = 1 << 21;
    /// Protection Key Enable
    pub const PKE: u64 = 1 << 22;
    /// Control-flow Enforcement Technology
    pub const CET: u64 = 1 << 23;
    /// Protection Keys for Supervisor-mode pages
    pub const PKS: u64 = 1 << 24;
    /// User Interrupts Enable
    pub const UINTR: u64 = 1 << 25;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtendedFeatureEnableReg;

#[allow(unused)]
impl ExtendedFeatureEnableReg {
    /// System Call Extensions
    pub const SCE: u64 = 1 << 0;
    /// Long Mode Enable
    pub const LME: u64 = 1 << 8;
    /// Long Mode Active
    pub const LMA: u64 = 1 << 10;
    /// No-Execute Enable
    pub const NXE: u64 = 1 << 11;
    /// Secure Virtual Machine Enable
    pub const SVME: u64 = 1 << 12;
    /// Long Mode Segment Limit Enable
    pub const LMSLE: u64 = 1 << 13;
    /// Fast FXSAVE/FXRSTOR
    pub const FFXSR: u64 = 1 << 14;
    /// Translation Cache Extension
    pub const TCE: u64 = 1 << 15;
    /// MCOMMIT Enable
    pub const MCOMMIT: u64 = 1 << 17;
    /// Interruptible WBINVD/WBNOINVD
    pub const INTWB: u64 = 1 << 18;
    /// Upper Address Ignore Enable
    pub const UAIE: u64 = 1 << 20;
    /// Automatic IBRS Enable
    pub const AIBRSE: u64 = 1 << 21;
}
