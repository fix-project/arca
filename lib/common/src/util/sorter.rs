extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use async_channel::{RecvError, SendError};
use core::cmp::Eq;
use core::fmt::Debug;
use core::hash::Hash;
use hashbrown::HashMap;

use crate::util::channel;
use crate::util::rwlock::RwLock;

#[derive(Debug)]
pub struct Sorter<K: Hash + Eq + Clone + Debug, V: Debug> {
    channels: Arc<RwLock<HashMap<K, channel::Sender<V>>>>,
}

impl<K: Hash + Eq + Clone + Debug, V: Debug> Default for Sorter<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Hash + Eq + Clone + Debug, V: Debug> Sorter<K, V> {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(Default::default())),
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
        self.channels.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn keys(&self) -> Vec<K> {
        self.channels.read().keys().cloned().collect()
    }
}

#[derive(Debug)]
pub struct Receiver<K: Hash + Eq + Clone + Debug, V: Debug> {
    port: K,
    rx: channel::Receiver<V>,
    channels: Arc<RwLock<HashMap<K, channel::Sender<V>>>>,
}

impl<K: Hash + Eq + Clone + Debug, V: Debug> Receiver<K, V> {
    pub async fn recv(&self) -> Result<V, RecvError> {
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
    channels: Arc<RwLock<HashMap<K, channel::Sender<V>>>>,
}

impl<K: Hash + Eq + Clone + Debug, V: Debug> Sender<K, V> {
    pub fn send_blocking(&mut self, port: K, value: V) -> Result<(), SendError<V>> {
        let channels = self.channels.read();
        let Some(channel) = channels.get(&port) else {
            return Err(SendError(value));
        };
        let result = channel.try_send(value);
        if result.is_err() {
            log::warn!("{port:?} was closed");
        }
        Ok(())
    }

    pub fn close(&mut self, port: K) {
        let mut channels = self.channels.lock();
        if let Some(channel) = channels.remove(&port) {
            channel.close();
        }
    }
}
