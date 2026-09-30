use core::fmt::Debug;
use core::future::Future;
use core::task::{Poll, Waker};

extern crate alloc;
use alloc::collections::VecDeque;
use alloc::sync::{Arc, Weak};

use crate::util::spinlock::SpinLock;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ChannelClosed;

impl core::fmt::Display for ChannelClosed {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "channel closed")
    }
}

impl core::error::Error for ChannelClosed {}

#[derive(Debug)]
struct Channel<T: Debug> {
    data: VecDeque<T>,
    wake_on_tx: VecDeque<Waker>,
}

pub fn unbounded<T: Debug>() -> (Sender<T>, Receiver<T>) {
    let channel = Arc::new(SpinLock::new(Channel {
        data: VecDeque::new(),
        wake_on_tx: VecDeque::new(),
    }));
    (
        Sender {
            channel: Arc::downgrade(&channel),
        },
        Receiver { channel },
    )
}

#[derive(Debug, Clone)]
pub struct Sender<T: Debug> {
    channel: Weak<SpinLock<Channel<T>>>,
}

impl<T: Debug> Sender<T> {
    pub fn send_blocking(&self, value: T) -> Result<(), ChannelClosed> {
        let channel = self.channel.upgrade().ok_or(ChannelClosed)?;
        let mut channel = channel.lock();
        channel.data.push_back(value);
        while let Some(waker) = channel.wake_on_tx.pop_front() {
            waker.wake()
        }
        Ok(())
    }

    pub fn receiver(&self) -> Result<Receiver<T>, ChannelClosed> {
        Ok(Receiver {
            channel: self.channel.upgrade().ok_or(ChannelClosed)?,
        })
    }

    pub fn close(self) {}
}

#[derive(Debug, Clone)]
pub struct Receiver<T: Debug> {
    channel: Arc<SpinLock<Channel<T>>>,
}

impl<T: Debug> Receiver<T> {
    pub async fn recv(&self) -> Result<T, ChannelClosed> {
        ReceiveFuture {
            channel: self.channel.clone(),
        }
        .await
    }

    pub fn try_recv(&self) -> Option<Result<T, ChannelClosed>> {
        let mut channel = self.channel.lock();
        channel.data.pop_front().map(Ok)
    }

    pub fn sender(&self) -> Sender<T> {
        Sender {
            channel: Arc::downgrade(&self.channel),
        }
    }

    pub fn close(self) {}
}

struct ReceiveFuture<T: Debug> {
    channel: Arc<SpinLock<Channel<T>>>,
}

impl<T: Debug> Future for ReceiveFuture<T> {
    type Output = Result<T, ChannelClosed>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let mut channel = self.channel.lock();
        if let Some(result) = channel.data.pop_front() {
            return Poll::Ready(Ok(result));
        }
        channel.wake_on_tx.push_back(cx.waker().clone());
        Poll::Pending
    }
}
