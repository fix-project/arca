use core::future::Future;
use core::task::{Poll, Waker};

extern crate alloc;
use alloc::collections::VecDeque;
use alloc::sync::Arc;

use crate::util::spinlock::SpinLock;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ChannelClosed;

#[derive(Debug)]
struct Channel<T> {
    closed: bool,
    data: VecDeque<T>,
    wake_on_tx: Option<Waker>,
}

pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
    let channel = Arc::new(SpinLock::new(Channel {
        closed: false,
        data: VecDeque::new(),
        wake_on_tx: None,
    }));
    (
        Sender {
            channel: channel.clone(),
        },
        Receiver { channel },
    )
}

#[derive(Debug)]
pub struct Sender<T> {
    channel: Arc<SpinLock<Channel<T>>>,
}

impl<T> Sender<T> {
    pub fn send_blocking(&self, value: T) -> Result<(), ChannelClosed> {
        let mut channel = self.channel.lock();
        if channel.closed {
            return Err(ChannelClosed);
        }
        channel.data.push_back(value);
        if let Some(waker) = channel.wake_on_tx.take() {
            waker.wake()
        }
        Ok(())
    }

    pub fn close(self) {}
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut channel = self.channel.lock();
        channel.closed = true;
    }
}

#[derive(Debug)]
pub struct Receiver<T> {
    channel: Arc<SpinLock<Channel<T>>>,
}

impl<T> Receiver<T> {
    pub async fn recv(&self) -> Result<T, ChannelClosed> {
        ReceiveFuture {
            channel: self.channel.clone(),
        }
        .await
    }

    pub fn try_recv(&self) -> Option<Result<T, ChannelClosed>> {
        let mut channel = self.channel.lock();
        if let Some(result) = channel.data.pop_front() {
            Some(Ok(result))
        } else if channel.closed {
            Some(Err(ChannelClosed))
        } else {
            None
        }
    }

    pub fn close(self) {}
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let mut channel = self.channel.lock();
        channel.closed = true;
    }
}

struct ReceiveFuture<T> {
    channel: Arc<SpinLock<Channel<T>>>,
}

impl<T> Future for ReceiveFuture<T> {
    type Output = Result<T, ChannelClosed>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let mut channel = self.channel.lock();
        if let Some(result) = channel.data.pop_front() {
            return Poll::Ready(Ok(result));
        }
        if channel.closed {
            return Poll::Ready(Err(ChannelClosed));
        }
        channel.wake_on_tx = Some(cx.waker().clone());
        Poll::Pending
    }
}
