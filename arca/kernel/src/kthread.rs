use core::{
    cell::RefCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

use alloc::{boxed::Box, sync::Arc};
use common::util::{initcell::LazyLock, spinlock::SpinLock};
use crossbeam_queue::SegQueue;

use crate::{interrupts::IO_GENERATION, page::Page2MB};

mod kcondvar;
pub mod kmutex;
pub use kcondvar::KCondvar;
pub use kmutex::{KMutex, KMutexGuard};

#[derive(Default, Debug)]
pub struct KThread {
    tid: usize,
    rsp: *const u8,
    base: *mut MaybeUninit<Page2MB>,
    scheduler: bool,
    exited: bool,
    wfi: bool,
    parked: bool,
    mutex_wait: Option<MutexWait>,
    thread: Arc<Thread>,
    io_generation: usize,
    next: Option<Box<KThread>>,
}

unsafe impl Send for KThread {}

unsafe extern "C" {
    fn cswitch(new_stack: *const u8, old_stack: *mut *const u8) -> *const u8;
    fn kthread_init();
}

static CURRENT_TID: AtomicUsize = AtomicUsize::new(0);
static OUTSTANDING: AtomicUsize = AtomicUsize::new(0);

static WFI: SpinLock<Option<Box<KThread>>> = SpinLock::new(None);
static IDLE: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];

// KMutex::lock keeps both pointees alive while its thread is suspended.
#[derive(Debug)]
struct MutexWait {
    locked: *const AtomicBool,
    waiters: *const SpinLock<Option<Box<KThread>>>,
}

fn wait_for_mutex(locked: &AtomicBool, waiters: &SpinLock<Option<Box<KThread>>>) {
    {
        let mut current = CURRENT_THREAD.borrow_mut();
        current.mutex_wait = Some(MutexWait { locked, waiters });
    }
    yield_now();
}

#[derive(Default, Debug)]
pub struct Thread {
    notified: AtomicBool,
    waiting: SpinLock<Option<Box<KThread>>>,
}

impl Thread {
    pub fn unpark(&self) {
        if !self.notified.swap(true, Ordering::AcqRel) {
            let waiting = self.waiting.lock().take();
            if let Some(mut thread) = waiting {
                thread.wfi = false;
                enqueue(thread);
            }
        }
    }
}

pub fn current() -> Arc<Thread> {
    CURRENT_THREAD.borrow().thread.clone()
}

pub fn park() {
    let thread = current();
    while !thread.notified.swap(false, Ordering::AcqRel) {
        {
            let mut current = CURRENT_THREAD.borrow_mut();
            current.wfi = true;
            current.parked = true;
        }
        yield_now();
    }
    CURRENT_THREAD.borrow_mut().parked = false;
}

fn next_tid() -> usize {
    CURRENT_TID.fetch_add(1, Ordering::SeqCst)
}

#[core_local]
pub static SCHEDULER_THREAD: LazyLock<RefCell<KThread>> = LazyLock::new(|| {
    RefCell::new(KThread {
        tid: next_tid(),
        rsp: core::ptr::null(),
        base: core::ptr::null_mut(),
        scheduler: true,
        exited: false,
        wfi: false,
        parked: false,
        mutex_wait: None,
        thread: Arc::default(),
        io_generation: 0,
        next: None,
    })
});

#[core_local]
pub static CURRENT_THREAD: LazyLock<RefCell<Box<KThread>>> = LazyLock::new(RefCell::default);

pub fn tid() -> usize {
    CURRENT_THREAD.borrow().tid
}

pub fn scheduler() -> bool {
    CURRENT_THREAD.borrow().scheduler
}

pub fn exited() -> bool {
    CURRENT_THREAD.borrow().exited
}

pub fn init() {
    LazyLock::force(&SCHEDULER_THREAD);
    LazyLock::force(&CURRENT_THREAD);
}

pub static THREAD_QUEUE: SegQueue<Box<KThread>> = SegQueue::new();

fn enqueue(thread: Box<KThread>) {
    THREAD_QUEUE.push(thread);
    wake_idle();
}

pub(crate) fn wake_idle() {
    for (index, idle) in IDLE.iter().enumerate() {
        let mut bits = idle.load(Ordering::SeqCst);
        while bits != 0 {
            let bit = bits.trailing_zeros();
            match idle.compare_exchange_weak(
                bits,
                bits & !(1 << bit),
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    crate::lapic::wake((index * 64 + bit as usize) as u32);
                    return;
                }
                Err(current) => bits = current,
            }
        }
    }
}

pub fn spawn(f: impl FnOnce() + Send + 'static) {
    let f: Box<Box<dyn FnOnce() + Send>> = Box::new(Box::new(f));
    let f = Box::into_raw(f);
    let tid = next_tid();
    let stack = Box::<Page2MB>::new_uninit();
    let base = Box::into_raw(stack);
    let stack = unsafe {
        &mut *core::mem::transmute::<
            *mut MaybeUninit<Page2MB>,
            *mut [MaybeUninit<u64>; (1 << 21) / 8],
        >(base)
    };
    stack[stack.len() - 1].write(f as u64); // data ptr
    stack[stack.len() - 2].write(start_thread as *const () as u64); // function ptr (aligned)
    stack[stack.len() - 3].write(kthread_init as *const () as u64); // return address
    stack[stack.len() - 4].write(0); // rbx (aligned)
    stack[stack.len() - 5].write(0); // rbp
    stack[stack.len() - 6].write(0); // r12 (aligned)
    stack[stack.len() - 7].write(0); // r13
    stack[stack.len() - 8].write(0); // r14 (aligned)
    stack[stack.len() - 9].write(0); // r15
    let rsp = &raw mut stack[stack.len() - 9] as *const u8;
    let thread = KThread {
        tid,
        rsp,
        base,
        scheduler: false,
        exited: false,
        wfi: false,
        parked: false,
        mutex_wait: None,
        thread: Arc::default(),
        io_generation: 0,
        next: None,
    };
    OUTSTANDING.fetch_add(1, Ordering::SeqCst);
    enqueue(thread.into());
}

/**
 * Waits for tasks to be scheduled, then runs tasks until there are none left.
 *
 * # Safety
 *
 * This function should only be called from rsstart; its calls should never be nested.
 */
pub(crate) unsafe fn run_scheduler() {
    let core = crate::coreid() as usize;
    let idle = &IDLE[core / 64];
    let bit = 1 << (core % 64);
    while OUTSTANDING.load(Ordering::SeqCst) == 0 {
        // wait for something to be scheduled
        core::hint::spin_loop();
    }
    while OUTSTANDING.load(Ordering::SeqCst) != 0 {
        // Keep interrupts disabled from the final readiness check through HLT.
        // STI's interrupt shadow makes STI; HLT indivisible with respect to IRQs.
        unsafe { crate::interrupts::disable() };
        // Publish idle before checking work; enqueue publishes work before waking.
        idle.fetch_or(bit, Ordering::SeqCst);
        let generation = IO_GENERATION.load(Ordering::SeqCst);
        if let Some(mut wfi) = WFI.try_lock() {
            let mut head = wfi.take();
            while let Some(mut current) = head {
                head = current.next.take();
                if current.io_generation != generation {
                    current.wfi = false;
                    enqueue(current);
                } else {
                    current.next = wfi.take();
                    wfi.replace(current);
                }
            }
        }
        let next = THREAD_QUEUE.pop();
        let Some(next) = next else {
            if generation == IO_GENERATION.load(Ordering::SeqCst) {
                sleep();
            }
            idle.fetch_and(!bit, Ordering::SeqCst);
            continue;
        };
        idle.fetch_and(!bit, Ordering::SeqCst);
        if !THREAD_QUEUE.is_empty() {
            wake_idle();
        }
        unsafe { crate::interrupts::enable() };
        log::trace!("scheduling thread {}", next.tid);
        CURRENT_THREAD.replace(next);
        let mut scheduler = SCHEDULER_THREAD.borrow_mut();
        let old_rsp = &raw mut scheduler.rsp;
        core::mem::drop(scheduler);
        let new_rsp = CURRENT_THREAD.borrow_mut().rsp;
        unsafe { cswitch(new_rsp, old_rsp) };
        let mut next = CURRENT_THREAD.take();
        if next.exited {
            log::trace!("thread {} exited", next.tid);
            let old_stack = next.base;
            let ptr = unsafe { Box::from_raw(old_stack) };
            core::mem::drop(ptr);
            let left = OUTSTANDING.fetch_sub(1, Ordering::SeqCst) - 1;
            log::trace!("{left} threads outstanding");
        } else if let Some(wait) = next.mutex_wait.take() {
            let mut waiters = unsafe { &*wait.waiters }.lock();
            if unsafe { &*wait.locked }.load(Ordering::Acquire) {
                next.next = waiters.take();
                *waiters = Some(next);
            } else {
                enqueue(next);
            }
        } else if next.parked {
            let thread = next.thread.clone();
            let mut waiting = thread.waiting.lock();
            if thread.notified.load(Ordering::Acquire) {
                next.wfi = false;
                enqueue(next);
            } else {
                *waiting = Some(next);
            }
        } else if next.wfi {
            log::trace!("thread {} is waiting", next.tid);
            let mut wfi = WFI.lock();
            if next.io_generation != IO_GENERATION.load(Ordering::SeqCst) {
                next.wfi = false;
                enqueue(next);
            } else {
                next.next = core::mem::take(&mut *wfi);
                wfi.replace(next);
            }
        } else {
            log::trace!("thread {} yielded", next.tid);
            enqueue(next);
        }
    }
}

pub fn yield_now() {
    log::trace!("exiting from {} into scheduler", tid());
    let next_rsp = SCHEDULER_THREAD.borrow_mut().rsp;
    let mut current = CURRENT_THREAD.borrow_mut();
    let current_rsp = &raw mut current.rsp;
    core::mem::drop(current);
    unsafe {
        cswitch(next_rsp, current_rsp);
    }
}

fn sleep() {
    unsafe {
        crate::interrupts::must_be_disabled();
        core::arch::asm!("sti", "hlt");
    }
}

fn exit() {
    CURRENT_THREAD.borrow_mut().exited = true;
    yield_now();
}

/// Wait until an I/O predicate is satisfied, including interrupts arriving
/// between checking the predicate and parking the thread.
pub fn wait_until(mut ready: impl FnMut() -> bool) {
    loop {
        let generation = IO_GENERATION.load(Ordering::SeqCst);
        if ready() {
            return;
        }
        {
            let mut current = CURRENT_THREAD.borrow_mut();
            current.io_generation = generation;
            current.wfi = true;
        }
        yield_now();
    }
}

unsafe extern "C" fn start_thread(f: *mut Box<dyn FnOnce() + Send>) {
    log::trace!("starting thread {}", tid());
    let f = unsafe { f.read() };
    f();
    exit();
}
