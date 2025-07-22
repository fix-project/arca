use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
    time::Duration,
};

use alloc::{
    boxed::Box,
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
    sync::Arc,
    task::Wake,
};
use common::util::rwlock::{ReadGuard, RwLock, WriteGuard};
use time::OffsetDateTime;

use crate::{kvmclock, prelude::*};

pub static EXECUTOR: LazyLock<Executor> = LazyLock::new(Executor::new);

pub struct Executor {
    pending: Arc<RwLock<VecDeque<Arc<Task>>>>,
    sleeping: RwLock<BTreeMap<OffsetDateTime, Waker>>,
    active: AtomicUsize,
    parallel: AtomicUsize,
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
        Executor {
            pending: Arc::new(RwLock::new(VecDeque::new())),
            sleeping: RwLock::new(BTreeMap::new()),
            active: AtomicUsize::new(0),
            parallel: AtomicUsize::new(0),
        }
    }

    #[inline(never)]
    async fn resolve<F, T>(future: F, entry: Arc<RwLock<Entry<T>>>)
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
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
            pending: self.pending.clone(),
        });
        let handle = JoinHandle { entry };
        self.active.fetch_add(1, Ordering::AcqRel);
        let mut tasks = self.pending.write();
        tasks.push_back(task);
        handle
    }

    #[inline(never)]
    fn wake_sleeping(&self) -> bool {
        let now = kvmclock::now();
        let sleeping = self.sleeping.read();
        let Some((first, _)) = sleeping.first_key_value() else {
            return false;
        };
        if first > &now {
            return false;
        }
        let mut sleeping = ReadGuard::upgrade(sleeping);
        let mut anything = false;
        while let Some(first) = sleeping.first_entry() {
            let now = kvmclock::now();
            if &now >= first.key() {
                let waker = first.remove();
                waker.wake_by_ref();
                anything = true;
            } else {
                break;
            }
        }
        anything
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
        let tasks = self.pending.read();
        if tasks.is_empty() {
            return false;
        }
        let mut tasks = ReadGuard::upgrade(tasks);
        let Some(task) = tasks.pop_front() else {
            return false;
        };
        WriteGuard::unlock(tasks);
        self.parallel.fetch_add(1, Ordering::SeqCst);
        TIME_SCHEDULING.fetch_add(self.diff(), Ordering::SeqCst);
        let result = task.poll();
        TIME_WORKING.fetch_add(self.diff(), Ordering::SeqCst);
        self.parallel.fetch_sub(1, Ordering::SeqCst);
        if result.is_ready() {
            self.active.fetch_sub(1, Ordering::AcqRel);
        }
        true
    }

    #[inline(never)]
    fn sleep(&self) {
        TIME_SCHEDULING.fetch_add(self.diff(), Ordering::SeqCst);
        unsafe {
            crate::profile::muted(|| {
                core::arch::asm!("hlt");
            });
        }
        TIME_SLEEPING.fetch_add(self.diff(), Ordering::SeqCst);
    }

    pub fn run(&self) {
        self.diff();
        while self.active.load(Ordering::Acquire) != 0 {
            crate::interrupts::must_be_enabled();
            let mut anything = false;
            anything |= self.wake_sleeping();
            anything |= self.run_pending();
            if !anything {
                self.sleep();
            }
        }
    }
}

pub struct JoinHandle<T> {
    entry: Arc<RwLock<Entry<T>>>,
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

pub fn run() {
    EXECUTOR.run();
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

struct Task {
    future: SpinLock<BoxFuture>,
    pending: Arc<RwLock<VecDeque<Arc<Task>>>>,
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
        let other = self.clone();
        let mut tasks = other.pending.write();
        tasks.push_back(self);
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
        let mut sleeping = EXECUTOR.sleeping.write();
        sleeping.insert(self.0, waker);
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
