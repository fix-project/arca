use core::arch::asm;

pub unsafe fn wrmsr(msr: u32, val: u64) {
    asm!(
        "wrmsr",
        in("edx")(val >> 32),
        in("eax")(val),
        in("ecx")(msr)
    );
}

pub unsafe fn rdmsr(msr: u32) -> u64 {
    let mut eax: u32;
    let mut edx: u32;
    asm!(
        "rdmsr",
        out("edx")(edx),
        out("eax")(eax),
        in("ecx")(msr)
    );
    ((edx as u64) << 32) | (eax as u64)
}
