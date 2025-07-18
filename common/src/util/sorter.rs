extern crate alloc;

use alloc::sync::Arc;
use core::cmp::Eq;
use core::hash::Hash;
use hashbrown::HashMap;

use crate::util::channel::{self, ChannelClosed};
use crate::util::spinlock::SpinLock;

#[derive(Debug)]
pub struct Sorter<K: Hash + Eq + Clone, V> {
    channels: Arc<SpinLock<HashMap<K, channel::Sender<V>>>>,
}

impl<K: Hash + Eq + Clone, V> Sorter<K, V> {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(SpinLock::new(Default::default())),
        }
    }

    pub fn sender(&self) -> Sender<K, V> {
        Sender {
            channels: self.channels.clone(),
        }
    }

    pub fn receiver(&self, port: K) -> Receiver<K, V> {
        let mut channels = self.channels.lock();
        let (tx, rx) = channel::unbounded();
        channels.insert(port.clone(), tx);
        Receiver {
            port,
            rx,
            channels: self.channels.clone(),
        }
    }
}

#[derive(Debug)]
pub struct Receiver<K: Hash + Eq + Clone, V> {
    port: K,
    rx: channel::Receiver<V>,
    channels: Arc<SpinLock<HashMap<K, channel::Sender<V>>>>,
}

impl<K: Hash + Eq + Clone, V> Receiver<K, V> {
    pub async fn recv(&mut self) -> Result<V, ChannelClosed> {
        self.rx.recv().await
    }
}

impl<K: Hash + Eq + Clone, V> Drop for Receiver<K, V> {
    fn drop(&mut self) {
        let mut channels = self.channels.lock();
        channels.remove(&self.port);
    }
}

#[derive(Clone)]
pub struct Sender<K: Hash + Eq + Clone, V> {
    channels: Arc<SpinLock<HashMap<K, channel::Sender<V>>>>,
}

impl<K: Hash + Eq + Clone, V> Sender<K, V> {
    pub fn send_blocking(&mut self, port: K, value: V) -> Result<(), ChannelClosed> {
        let mut channels = self.channels.lock();
        let channel = channels.get_mut(&port).ok_or(ChannelClosed)?;
        let result = channel.send_blocking(value);
        if result.is_err() {
            channels.remove(&port);
        }
        result
    }
}
