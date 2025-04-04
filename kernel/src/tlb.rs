use alloc::boxed::Box;
use common::util::initcell::LazyLock;
use core::{
    cell::{LazyCell, RefCell},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use crate::{
    kvmclock,
    lapic::{write, write64},
};

pub static TLB_SHOOTDOWN_ENABLED: AtomicBool = AtomicBool::new(false);

static TLB_SHOOTDOWN: LazyLock<Box<[AtomicBool]>> =
    LazyLock::new(|| unsafe { Box::new_zeroed_slice(crate::ncores()).assume_init() });
pub static ACTIVE_CORES: LazyLock<Box<[AtomicBool]>> =
    LazyLock::new(|| unsafe { Box::new_zeroed_slice(crate::ncores()).assume_init() });

#[core_local]
pub static AWAITING: LazyCell<RefCell<Box<[bool]>>> =
    LazyCell::new(|| RefCell::new(unsafe { Box::new_zeroed_slice(crate::ncores()).assume_init() }));

pub fn set_enabled(val: bool) {
    TLB_SHOOTDOWN_ENABLED.store(val, Ordering::Release);
}

fn is_enabled() -> bool {
    TLB_SHOOTDOWN_ENABLED.load(Ordering::Acquire)
}

unsafe fn fire(core: u32) -> bool {
    if is_sleeping(core) {
        // set_pending(core, true);
        false
    } else {
        if !get_and_set_pending(core, true) {
            write64(0x30, 0b11 << 18 | 0x30 | 0b11 << 14);
        }
        true
    }
}

// #[allow(unused)]
// unsafe fn nmi(core: u32) {
//     // let this = crate::coreid();
//     // log::warn!("{this} sending NMI to {core}");
//     if is_sleeping(core) {
//         return;
//     }
//     write64(0x30, (core as u64) << 32 | 0b11 << 14 | 4 << 8);
// }

unsafe fn fire_all(cores: &mut [bool]) -> usize {
    cores.fill(false);
    let mut count = 0;
    for core in 0..crate::ncores() as u32 {
        if fire(core) {
            cores[core as usize] = true;
            count += 1;
        }
    }
    count
}

unsafe fn wait_for(core: u32) -> bool {
    let mut last = kvmclock::time_since_boot();
    loop {
        if is_sleeping(core) {
            return false;
        }
        if !is_pending(core) {
            return true;
        }
        let now = kvmclock::time_since_boot();
        if now - last > Duration::from_micros(10) {
            // fire(core);
            last = now;
        }
        core::hint::spin_loop();
    }
}

unsafe fn wait_for_all(cores: &mut [bool]) -> (usize, usize) {
    let mut interrupted = 0;
    let mut sleeping = 0;
    for (core, needed) in cores.iter_mut().enumerate() {
        if *needed {
            if wait_for(core as u32) {
                interrupted += 1;
            } else {
                sleeping += 1;
            }
            *needed = false;
        }
    }
    (interrupted, sleeping)
}

pub unsafe fn shootdown() {
    if !is_enabled() {
        return;
    }
    while_sleeping(|| {
        let mut awaiting = AWAITING.borrow_mut();
        let needed = fire_all(&mut awaiting);
        if needed == 0 {
            return;
        }
        // let start = kvmclock::time_since_boot();
        // let (interrupted, sleeping) = wait_for_all(&mut awaiting);
        wait_for_all(&mut awaiting);
        // let end = kvmclock::time_since_boot();
        // let time = end - start;
        // if time > Duration::from_millis(1) {
        //     log::info!("waiting for {needed} threads took {time:?} ({interrupted} interrupted, {sleeping} sleeping)");
        // }
    });
}

extern "C" {
    fn set_pt(page_map: usize);
    fn get_pt() -> usize;
}

pub unsafe fn flush_if_needed() {
    let this = crate::coreid();
    if is_pending(this) {
        set_pt(get_pt());
        set_pending(this, false);
    }
}

pub unsafe fn handle_shootdown() {
    flush_if_needed();
    write(0x0B, 0);
}

pub fn is_sleeping(core: u32) -> bool {
    !ACTIVE_CORES[core as usize].load(Ordering::Acquire)
}

pub fn is_pending(core: u32) -> bool {
    TLB_SHOOTDOWN[core as usize].load(Ordering::Acquire)
}

pub unsafe fn set_sleeping(sleeping: bool) {
    let this = crate::coreid();
    ACTIVE_CORES[this as usize].store(!sleeping, Ordering::Release);
}

pub unsafe fn get_and_set_sleeping(sleeping: bool) -> bool {
    let this = crate::coreid();
    let old = !ACTIVE_CORES[this as usize].load(Ordering::Acquire);
    ACTIVE_CORES[this as usize].store(!sleeping, Ordering::Release);
    old
}

pub unsafe fn set_pending(core: u32, pending: bool) {
    TLB_SHOOTDOWN[core as usize].store(pending, Ordering::Release);
}

pub unsafe fn get_and_set_pending(core: u32, pending: bool) -> bool {
    let old = TLB_SHOOTDOWN[core as usize].load(Ordering::Acquire);
    TLB_SHOOTDOWN[core as usize].store(pending, Ordering::Release);
    old
}

#[inline(always)]
pub fn while_sleeping<T>(f: impl FnOnce() -> T) -> T {
    if is_enabled() {
        unsafe {
            let old = get_and_set_sleeping(true);
            let result = f();
            flush_if_needed();
            assert!(is_sleeping(crate::coreid()));
            set_sleeping(old);
            result
        }
    } else {
        f()
    }
}
