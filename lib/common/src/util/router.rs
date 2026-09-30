extern crate alloc;

use alloc::sync::Arc;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use crate::util::concurrent_trie::Trie;

#[derive(Clone)]
pub struct Router<V> {
    trie: Arc<Trie<2, Entry<V>>>,
}

enum Entry<V> {
    Waker(Waker),
    Sent(V),
}

pub struct ReceiveFuture<V> {
    key: u64,
    trie: Arc<Trie<2, Entry<V>>>,
}

impl<V> Default for Router<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> Router<V> {
    pub fn new() -> Self {
        Self {
            trie: Arc::new(Trie::new()),
        }
    }

    pub fn send(&self, key: u64, value: V) {
        if let Some(entry) = self.trie.insert(key, Entry::Sent(value)) {
            let Entry::Waker(waker) = entry else {
                panic!("double send");
            };
            waker.wake();
        }
    }

    pub fn recv(&self, key: u64) -> ReceiveFuture<V> {
        ReceiveFuture {
            key,
            trie: self.trie.clone(),
        }
    }
}

impl<V> Future for ReceiveFuture<V> {
    type Output = V;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let entry = Entry::Waker(cx.waker().clone());
        if let Some(entry) = self.trie.insert(self.key, entry) {
            let Entry::Sent(sent) = entry else {
                panic!("double receive");
            };
            assert!(self.trie.remove(self.key).is_some());
            Poll::Ready(sent)
        } else {
            Poll::Pending
        }
    }
}
