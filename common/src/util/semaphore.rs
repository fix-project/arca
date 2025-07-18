extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::future::Future;
use core::task::{Poll, Waker};

use crate::util::spinlock::SpinLock;

pub struct Semaphore {
    inner: Arc<SpinLock<Inner>>,
}

struct Inner {
    current: usize,
    wakers: Vec<Waker>,
}

impl Semaphore {
    pub fn new(count: usize) -> Self {
        Semaphore {
            inner: Arc::new(SpinLock::new(Inner {
                current: count,
                wakers: Vec::new(),
            })),
        }
    }

    pub async fn acquire(&self, count: usize) {
        SemaphoreFuture {
            count,
            inner: self.inner.clone(),
        }
        .await
    }

    pub fn release(&self, count: usize) {
        let wakers = {
            let mut inner = self.inner.lock();
            inner.current += count;
            core::mem::take(&mut inner.wakers)
        };
        for w in wakers.into_iter() {
            w.wake()
        }
    }
}

struct SemaphoreFuture {
    count: usize,
    inner: Arc<SpinLock<Inner>>,
}

impl Future for SemaphoreFuture {
    type Output = ();

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let mut inner = self.inner.lock();
        if inner.current >= self.count {
            inner.current -= self.count;
            return Poll::Ready(());
        }
        inner.wakers.push(cx.waker().clone());
        Poll::Pending
    }
}
