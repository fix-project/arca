use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
    time::Duration,
};

use alloc::{boxed::Box, sync::Arc, task::Wake};
use common::util::{
    channel::{Receiver, Sender},
    concurrent_trie::Trie,
    rwlock::RwLock,
};
use time::OffsetDateTime;

use crate::{interrupts::INTERRUPTED, io, kvmclock, prelude::*};

pub static EXECUTOR: LazyLock<Executor> = LazyLock::new(Executor::new);

pub static SHUTDOWN: AtomicBool = AtomicBool::new(false);

pub struct Executor {
    todo_rx: Receiver<Arc<Task>>,
    todo_tx: Sender<Arc<Task>>,
    sleeping: Trie<2, Waker>,
    wfi_rx: Receiver<Waker>,
    wfi_tx: Sender<Waker>,
    active: AtomicUsize,
}

type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

#[core_local]
static LAST_TIME: SpinLock<u128> = SpinLock::new(0);

static TIME_SLEEPING: AtomicUsize = AtomicUsize::new(0);
static TIME_WORKING: AtomicUsize = AtomicUsize::new(0);
static TIME_SCHEDULING: AtomicUsize = AtomicUsize::new(0);

enum Entry<T> {
    Nothing,
    Finished(Option<T>),
    Waiting(Waker),
}

impl Executor {
    fn new() -> Executor {
        let (todo_tx, todo_rx) = channel::unbounded();
        let (wfi_tx, wfi_rx) = channel::unbounded();
        Executor {
            todo_rx,
            todo_tx,
            sleeping: Trie::default(),
            wfi_rx,
            wfi_tx,
            active: AtomicUsize::new(0),
        }
    }

    #[inline(never)]
    async fn resolve<F, T>(future: F, entry: Arc<RwLock<Entry<T>>>)
    where
        F: Future<Output = T> + Send,
        T: Send,
    {
        let result = future.await;
        let mut entry = entry.write();
        let mut replacement = Entry::Finished(Some(result));
        core::mem::swap(&mut *entry, &mut replacement);
        match replacement {
            Entry::Nothing => {}
            Entry::Finished(_) => unreachable!(),
            Entry::Waiting(waker) => {
                waker.wake();
            }
        }
    }

    pub fn spawn<F, T>(&self, future: F) -> JoinHandle<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let entry = Arc::new(RwLock::new(Entry::Nothing));
        let future = Self::resolve(future, entry.clone());
        let task = Arc::new(Task {
            future: SpinLock::new(Box::pin(future)),
            todo: self.todo_tx.clone(),
        });
        let handle = JoinHandle { entry };
        self.active.fetch_add(1, Ordering::AcqRel);
        self.todo_tx.try_send(task).unwrap();
        handle
    }

    pub fn spawn_blocking<'a, F, T>(&self, future: F) -> T
    where
        F: Future<Output = T> + Send + 'a,
        T: Send + 'a,
    {
        let entry = Arc::new(RwLock::new(Entry::Nothing));
        let future = Self::resolve(future, entry.clone());
        unsafe {
            let pin: Pin<Box<dyn Future<Output = ()> + Send + 'a>> = Box::pin(future);
            let pin: BoxFuture = core::mem::transmute(pin);
            let task = Arc::new(Task {
                future: SpinLock::new(pin),
                todo: self.todo_tx.clone(),
            });
            let handle = JoinHandle { entry };
            self.active.fetch_add(1, Ordering::AcqRel);
            self.todo_tx.try_send(task).unwrap();
            handle.join()
        }
    }

    #[inline(never)]
    fn wake_sleeping(&self) -> bool {
        let Some(key) = self.sleeping.first_key() else {
            // No first key found. If we missed one because of a race it's not the end of the world anyway.
            return false;
        };
        let now = kvmclock::now();
        let timestamp = now.unix_timestamp() as u64;
        if key < timestamp {
            // The first task is still in the future.
            return false;
        }
        let Some(value) = self.sleeping.remove(key) else {
            // We saw a first key, but someone else grabbed it first. They can deal with this.
            return false;
        };
        value.wake();
        true
    }

    fn diff(&self) -> usize {
        let mut time = LAST_TIME.lock();
        let now = kvmclock::time_since_boot().as_nanos();
        let diff = now.wrapping_sub(*time);
        *time = now;
        diff as usize
    }

    #[inline(never)]
    fn run_pending(&self) -> bool {
        let Ok(task) = self.todo_rx.try_recv() else {
            return false;
        };
        TIME_SCHEDULING.fetch_add(self.diff(), Ordering::SeqCst);
        let result = task.poll();
        TIME_WORKING.fetch_add(self.diff(), Ordering::SeqCst);
        if result.is_ready() {
            self.active.fetch_sub(1, Ordering::AcqRel);
        }
        true
    }

    #[inline(never)]
    fn sleep(&self) {
        if !INTERRUPTED.load(Ordering::Relaxed) {
            TIME_SCHEDULING.fetch_add(self.diff(), Ordering::SeqCst);
            unsafe {
                crate::profile::muted(|| {
                    io::outl(0xf4, 0);
                    core::arch::asm!("hlt");
                });
            }
            TIME_SLEEPING.fetch_add(self.diff(), Ordering::SeqCst);
        }
    }

    #[inline(never)]
    fn wake_interrupted(&self) -> bool {
        let mut anything = false;
        if INTERRUPTED
            .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            while let Ok(waker) = self.wfi_rx.try_recv() {
                anything = true;
                waker.wake();
            }
        }
        anything
    }

    pub fn tick(&self) {
        crate::interrupts::must_be_enabled();
        let mut anything = false;
        anything |= self.wake_interrupted();
        anything |= self.wake_sleeping();
        anything |= self.run_pending();
        if !anything {
            self.sleep();
        }
    }

    pub fn run(&self) {
        self.diff();
        while self.active.load(Ordering::Acquire) != 0 {
            self.tick();
        }
    }
}

pub struct JoinHandle<T> {
    entry: Arc<RwLock<Entry<T>>>,
}

impl<T> JoinHandle<T> {
    fn join(self) -> T {
        loop {
            if let Some(mut entry) = self.entry.try_write() {
                if let Entry::Finished(value) = &mut *entry {
                    return value.take().unwrap();
                }
            }
            EXECUTOR.tick();
        }
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut result = self.entry.write();
        match &mut *result {
            Entry::Nothing | Entry::Waiting(_) => {
                *result = Entry::Waiting(cx.waker().clone());
                Poll::Pending
            }
            Entry::Finished(value) => Poll::Ready(value.take().unwrap()),
        }
    }
}

pub fn spawn<F, T>(future: F) -> JoinHandle<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    EXECUTOR.spawn(future)
}

pub fn spawn_blocking<F, T>(future: F) -> T
where
    F: Future<Output = T> + Send,
    T: Send,
{
    EXECUTOR.spawn_blocking(future)
}

pub fn run() {
    EXECUTOR.run();
}

struct Task {
    future: SpinLock<BoxFuture>,
    todo: Sender<Arc<Task>>,
}

impl Task {
    fn poll(self: Arc<Self>) -> Poll<()> {
        let waker = self.clone().into();
        let mut cx = Context::from_waker(&waker);
        let mut task_future = self.future.lock();
        task_future.as_mut().poll(&mut cx)
    }
}

impl Wake for Task {
    fn wake(self: Arc<Self>) {
        let todo = self.todo.clone();
        todo.try_send(self).unwrap();
    }
}

struct Yield(AtomicBool);

impl Future for Yield {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let done = self.0.load(Ordering::Acquire);
        match done {
            true => Poll::Ready(()),
            false => {
                self.0.store(true, Ordering::Release);
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}

pub fn yield_now() -> impl Future<Output = ()> {
    Yield(AtomicBool::new(false))
}

struct Delay(OffsetDateTime);

impl Future for Delay {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let now = kvmclock::now();
        if now >= self.0 {
            return Poll::Ready(());
        }
        let waker = cx.waker().clone();
        EXECUTOR
            .sleeping
            .insert(self.0.unix_timestamp() as u64, waker);
        Poll::Pending
    }
}

pub fn delay_for(duration: Duration) -> impl Future<Output = ()> {
    delay_until(kvmclock::now() + duration)
}

pub fn delay_until(time: OffsetDateTime) -> impl Future<Output = ()> {
    Delay(time)
}

pub fn reset_stats() {
    TIME_SLEEPING.store(0, Ordering::SeqCst);
    TIME_WORKING.store(0, Ordering::SeqCst);
    TIME_SCHEDULING.store(0, Ordering::SeqCst);
}

pub fn profile() {
    let scheduling = TIME_SCHEDULING.load(Ordering::SeqCst) as f64;
    let working = TIME_WORKING.load(Ordering::SeqCst) as f64;
    let sleeping = TIME_SLEEPING.load(Ordering::SeqCst) as f64;
    let total = scheduling + working + sleeping;
    log::info!(
        "time spent sleeping:   {sleeping:12} ({:3.2}%)",
        sleeping * 100. / total
    );
    log::info!(
        "time spent working:    {working:12} ({:3.2}%)",
        working * 100. / total
    );
    log::info!(
        "time spent scheduling: {scheduling:12} ({:3.2}%)",
        scheduling * 100. / total
    );
}

struct WaitForInterrupt {
    done: AtomicBool,
    wfi: Sender<Waker>,
}

impl Future for WaitForInterrupt {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let done = self.done.load(Ordering::Acquire);
        match done {
            true => Poll::Ready(()),
            false => {
                self.done.store(true, Ordering::Release);
                self.wfi.try_send(cx.waker().clone()).unwrap();
                Poll::Pending
            }
        }
    }
}

pub fn wfi() -> impl Future<Output = ()> {
    WaitForInterrupt {
        done: AtomicBool::new(false),
        wfi: EXECUTOR.wfi_tx.clone(),
    }
}
