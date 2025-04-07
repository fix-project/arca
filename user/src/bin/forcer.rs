#![no_std]
#![no_main]

extern crate user;

pub fn now() -> f64 {
    unsafe {
        let tsc = core::arch::x86_64::_rdtsc();
        let tsc_hz = u32::MAX as f64;
        tsc as f64 / tsc_hz
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    user::syscall::resize(2);
    user::syscall::prompt(1); // Thunk
    let warmup = 1.;
    let start = now();
    loop {
        user::syscall::force(1);
        if now() - start > warmup {
            break;
        }
    }
    let duration = 10.;
    let start = now();
    let mut iters: u64 = 0;
    loop {
        user::syscall::force(1);
        if now() - start > duration {
            user::syscall::create_word(0, iters);
            user::syscall::exit(0);
        }
        iters += 1;
    }
}
