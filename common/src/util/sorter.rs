extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::Eq;
use core::fmt::Debug;
use core::hash::Hash;
use hashbrown::HashMap;

use crate::util::channel::{self, ChannelClosed};
use crate::util::spinlock::SpinLock;

#[derive(Debug)]
pub struct Sorter<K: Hash + Eq + Clone + Debug, V: Debug> {
    channels: Arc<SpinLock<HashMap<K, channel::Sender<V>>>>,
}

impl<K: Hash + Eq + Clone + Debug, V: Debug> Default for Sorter<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Hash + Eq + Clone + Debug, V: Debug> Sorter<K, V> {
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
        let old = channels.insert(port.clone(), tx);
        assert!(old.is_none());
        Receiver {
            port,
            rx,
            channels: self.channels.clone(),
        }
    }

    pub fn try_receiver(&self, port: K) -> Option<Receiver<K, V>> {
        let mut channels = self.channels.lock();
        let (tx, rx) = channel::unbounded();
        if channels.contains_key(&port) {
            return None;
        }
        channels.insert(port.clone(), tx);
        Some(Receiver {
            port,
            rx,
            channels: self.channels.clone(),
        })
    }

    pub fn len(&self) -> usize {
        self.channels.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn keys(&self) -> Vec<K> {
        self.channels.lock().keys().cloned().collect()
    }
}

#[derive(Debug)]
pub struct Receiver<K: Hash + Eq + Clone + Debug, V: Debug> {
    port: K,
    rx: channel::Receiver<V>,
    channels: Arc<SpinLock<HashMap<K, channel::Sender<V>>>>,
}

impl<K: Hash + Eq + Clone + Debug, V: Debug> Receiver<K, V> {
    pub async fn recv(&self) -> Result<V, ChannelClosed> {
        self.rx.recv().await
    }

    pub fn key(&self) -> &K {
        &self.port
    }
}

impl<K: Hash + Eq + Clone + Debug, V: Debug> Drop for Receiver<K, V> {
    fn drop(&mut self) {
        let mut channels = self.channels.lock();
        channels.remove(&self.port);
    }
}

#[derive(Clone)]
pub struct Sender<K: Hash + Eq + Clone + Debug, V: Debug> {
    channels: Arc<SpinLock<HashMap<K, channel::Sender<V>>>>,
}

impl<K: Hash + Eq + Clone + Debug, V: Debug> Sender<K, V> {
    pub fn send_blocking(&mut self, port: K, value: V) -> Result<(), ChannelClosed> {
        let mut channels = self.channels.lock();
        let channel = channels.get_mut(&port).ok_or(ChannelClosed)?;
        let result = channel.send_blocking(value);
        if result.is_err() {
            log::warn!("{port:?} was closed");
        }
        result
    }

    pub fn close(&mut self, port: K) {
        let mut channels = self.channels.lock();
        if let Some(channel) = channels.remove(&port) {
            channel.close();
        }
    }
}
