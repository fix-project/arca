use crate::util::spinlock::SpinLock;
use core::{
    future::Future,
    task::{Poll, Waker},
};

extern crate alloc;
use alloc::sync::Arc;

#[derive(Debug)]
struct OneShot<T> {
    waker: Option<Waker>,
    value: Option<T>,
}

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let cell = Arc::new(SpinLock::new(OneShot {
        waker: None,
        value: None,
    }));
    (Sender { cell: cell.clone() }, Receiver { cell })
}

#[derive(Clone, Debug)]
pub struct Receiver<T> {
    cell: Arc<SpinLock<OneShot<T>>>,
}

impl<T> Receiver<T> {
    pub fn new(value: T) -> Self {
        let cell = Arc::new(SpinLock::new(OneShot {
            waker: None,
            value: Some(value),
        }));
        Self { cell }
    }

    pub fn get(&self) -> Option<T>
    where
        T: Copy,
    {
        let cell = self.cell.lock();
        cell.value
    }

    pub fn take(&mut self) -> Option<T> {
        let mut cell = self.cell.lock();
        cell.value.take()
    }
}

impl<T> Future for Receiver<T> {
    type Output = T;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let mut cell = self.cell.lock();
        if let Some(result) = cell.value.take() {
            return Poll::Ready(result);
        }
        assert!(cell.waker.is_none());
        cell.waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

#[derive(Clone, Debug)]
pub struct Sender<T> {
    cell: Arc<SpinLock<OneShot<T>>>,
}

impl<T> Sender<T> {
    pub fn send(self, value: T) {
        let mut cell = self.cell.lock();
        assert!(cell.value.is_none());
        cell.value = Some(value);
        if let Some(waker) = cell.waker.take() {
            waker.wake();
        }
    }
}

impl<T> FnOnce<(T,)> for Sender<T> {
    type Output = ();

    extern "rust-call" fn call_once(self, args: (T,)) -> Self::Output {
        self.send(args.0);
    }
}

impl<T> From<T> for Receiver<T> {
    fn from(value: T) -> Self {
        Receiver::new(value)
    }
}

impl<T, E> From<T> for Receiver<Result<T, E>> {
    fn from(value: T) -> Self {
        Receiver::new(Ok(value))
    }
}

impl<T> From<T> for Receiver<Option<T>> {
    fn from(value: T) -> Self {
        Receiver::new(Some(value))
    }
}
