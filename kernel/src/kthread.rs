use core::{
    cell::RefCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicUsize, Ordering},
};

use alloc::{boxed::Box, collections::vec_deque::VecDeque};
use common::util::{initcell::LazyLock, spinlock::SpinLock};

use crate::page::Page2MB;

#[derive(Default)]
pub struct KThread {
    tid: usize,
    rsp: *const u8,
    base: *mut MaybeUninit<Page2MB>,
    scheduler: bool,
    exited: bool,
}

unsafe impl Send for KThread {}

unsafe extern "C" {
    fn cswitch(new_stack: *const u8, old_stack: *mut *const u8) -> *const u8;
    fn kthread_init();
}

static CURRENT_TID: AtomicUsize = AtomicUsize::new(0);
static OUTSTANDING: AtomicUsize = AtomicUsize::new(0);

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
    })
});

#[core_local]
pub static CURRENT_THREAD: LazyLock<RefCell<KThread>> = LazyLock::new(RefCell::default);

pub fn tid() -> usize {
    CURRENT_THREAD.borrow().tid
}

pub fn scheduler() -> bool {
    CURRENT_THREAD.borrow().scheduler
}

pub fn exited() -> bool {
    CURRENT_THREAD.borrow().exited
}

pub unsafe fn init() {
    LazyLock::force(&SCHEDULER_THREAD);
    LazyLock::force(&CURRENT_THREAD);
}

pub static THREAD_QUEUE: LazyLock<SpinLock<VecDeque<KThread>>> =
    LazyLock::new(|| SpinLock::new(VecDeque::new()));

pub fn spawn(f: impl FnOnce()) {
    let f: Box<Box<dyn FnOnce()>> = Box::new(Box::new(f));
    let f = Box::into_raw(f);
    let tid = next_tid();
    let stack = Box::<Page2MB>::new_uninit();
    let base = Box::into_raw(stack);
    let stack: &mut [MaybeUninit<u64>; (1 << 21) / 8] = unsafe { core::mem::transmute(base) };
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
    };
    let mut q = THREAD_QUEUE.lock();
    OUTSTANDING.fetch_add(1, Ordering::SeqCst);
    q.push_back(thread);
}

pub unsafe fn run_scheduler() {
    while OUTSTANDING.load(Ordering::SeqCst) == 0 {
        // wait for something to be scheduled
    }
    while OUTSTANDING.load(Ordering::SeqCst) != 0 {
        let Some(next) = THREAD_QUEUE.lock().pop_front() else {
            sleep();
            continue;
        };
        log::trace!("scheduling thread {}", next.tid);
        CURRENT_THREAD.replace(next);
        let mut scheduler = SCHEDULER_THREAD.borrow_mut();
        let old_rsp = &raw mut scheduler.rsp;
        core::mem::drop(scheduler);
        let new_rsp = CURRENT_THREAD.borrow_mut().rsp;
        cswitch(new_rsp, old_rsp);
        let next = CURRENT_THREAD.take();
        if next.exited {
            log::trace!("thread {} exited", next.tid);
            let old_stack = next.base;
            let ptr = Box::from_raw(old_stack);
            core::mem::drop(ptr);
            let left = OUTSTANDING.fetch_sub(1, Ordering::SeqCst) - 1;
            log::trace!("{left} threads outstanding");
        } else {
            log::trace!("thread {} yielded", next.tid);
            THREAD_QUEUE.lock().push_back(next);
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
        // io::outl(0xf4, 0);
        core::arch::asm!("hlt");
    }
}

fn exit() {
    CURRENT_THREAD.borrow_mut().exited = true;
    yield_now();
}

unsafe extern "C" fn start_thread(f: *mut Box<dyn FnOnce()>) {
    log::trace!("starting thread {}", tid());
    let f = unsafe { f.read() };
    f();
    exit();
}
