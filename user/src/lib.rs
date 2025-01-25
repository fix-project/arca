#![no_std]
#![allow(unused)]

pub mod syscall {
    use core::arch::asm;

    pub const SYS_EXIT: u32 = 0;

    #[inline(always)]
    unsafe extern "C" fn syscall0(num: u64) -> u64 {
        unsafe {
            let mut val: u64;
            asm!("syscall", in("rdi")num, out("rax")val, out("rcx")_, out("r11")_);
            val
        }
    }

    #[inline(always)]
    unsafe extern "C" fn syscall1(num: u64, a0: u64) -> u64 {
        unsafe {
            let mut val: u64;
            asm!("syscall", in("rdi")num, in("rsi")a0, out("rax")val, out("rcx")_, out("r11")_);
            val
        }
    }

    #[inline(always)]
    unsafe extern "C" fn syscall2(num: u64, a0: u64, a1: u64) -> u64 {
        unsafe {
            let mut val: u64;
            asm!("syscall", in("rdi")num, in("rsi")a0, in("rdx")a1, out("rax")val, out("rcx")_, out("r11")_);
            val
        }
    }

    pub fn exit() -> ! {
        loop {
            unsafe { syscall0(SYS_EXIT.into()) };
            core::hint::spin_loop();
        }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
