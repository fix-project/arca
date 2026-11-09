extern crate alloc;

use crate::user_queues::Error;
use alloc::{boxed::Box, vec::Vec};
use core::cmp::Ordering;

type Link<K, V> = Option<Box<Node<K, V>>>;

struct Node<K, V> {
    key: K,
    version: u64,
    value: V,
    children: [Link<K, V>; 2],
    height: u8,
}

impl<K, V> Node<K, V> {
    fn height(link: &Link<K, V>) -> u8 {
        link.as_ref().map_or(0, |node| node.height)
    }
    fn update_height(&mut self) {
        self.height = 1 + Self::height(&self.children[0]).max(Self::height(&self.children[1]));
    }
    fn rotate(link: &mut Link<K, V>, upper: usize) {
        let mut lower = link.take().unwrap();
        let mut root = lower.children[upper].take().unwrap();
        lower.children[upper] = root.children[1 - upper].take();
        lower.update_height();
        root.children[1 - upper] = Some(lower);
        root.update_height();
        *link = Some(root);
    }
    fn balance(link: &mut Link<K, V>) {
        let node = link.as_mut().unwrap();
        node.update_height();
        for side in 0..2 {
            if Self::height(&node.children[side]) > Self::height(&node.children[1 - side]) + 1 {
                let child = node.children[side].as_ref().unwrap();
                if Self::height(&child.children[1 - side]) > Self::height(&child.children[side]) {
                    Self::rotate(&mut node.children[side], 1 - side);
                }
                Self::rotate(link, side);
                return;
            }
        }
    }
    fn search(
        link: &Link<K, V>,
        mut compare: impl FnMut(&K) -> Result<Ordering, Error>,
    ) -> Result<(Vec<usize>, bool), Error> {
        let mut path = Vec::new();
        let mut current = link;
        while let Some(node) = current {
            let side = match compare(&node.key)? {
                Ordering::Equal => return Ok((path, true)),
                Ordering::Less => 0,
                Ordering::Greater => 1,
            };
            path.push(side);
            current = &node.children[side];
        }
        Ok((path, false))
    }
    fn insert(link: &mut Link<K, V>, path: &[usize], node: Box<Self>) {
        if let Some((&side, rest)) = path.split_first() {
            Self::insert(&mut link.as_mut().unwrap().children[side], rest, node);
            Self::balance(link);
        } else {
            assert!(link.is_none());
            *link = Some(node);
        }
    }
    fn take_min(link: &mut Link<K, V>) -> Box<Self> {
        if link.as_ref().unwrap().children[0].is_none() {
            let mut node = link.take().unwrap();
            *link = node.children[1].take();
            node
        } else {
            let node = Self::take_min(&mut link.as_mut().unwrap().children[0]);
            Self::balance(link);
            node
        }
    }
    fn remove(link: &mut Link<K, V>, path: &[usize]) -> Box<Self> {
        if let Some((&side, rest)) = path.split_first() {
            let node = Self::remove(&mut link.as_mut().unwrap().children[side], rest);
            Self::balance(link);
            node
        } else {
            let mut node = link.take().unwrap();
            *link = if node.children[1].is_none() {
                node.children[0].take()
            } else if node.children[0].is_none() {
                node.children[1].take()
            } else {
                let mut root = Self::take_min(&mut node.children[1]);
                root.children = [node.children[0].take(), node.children[1].take()];
                Some(root)
            };
            if link.is_some() {
                Self::balance(link);
            }
            node
        }
    }
}

pub struct Maps<K, V> {
    maps: Vec<Link<K, V>>,
    version: u64,
}

impl<K, V> Default for Maps<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> Maps<K, V> {
    pub fn new() -> Self {
        Self {
            maps: Vec::new(),
            version: 0,
        }
    }
    pub fn count(&self) -> usize {
        self.maps.len()
    }
    pub fn create(&mut self) -> usize {
        let index = self.maps.len();
        self.maps.push(None);
        index
    }
    pub fn get(
        &self,
        map: usize,
        mut compare: impl FnMut(&K) -> Result<Ordering, Error>,
    ) -> Result<(u64, &V), Error> {
        let mut current = self.maps.get(map).ok_or(Error::Unauthorized)?;
        while let Some(node) = current {
            match compare(&node.key)? {
                Ordering::Equal => return Ok((node.version, &node.value)),
                order => current = &node.children[usize::from(order == Ordering::Greater)],
            }
        }
        Err(Error::Empty)
    }
    pub fn replace(
        &mut self,
        map: usize,
        key: K,
        expected: u64,
        value: V,
        compare: impl FnMut(&K) -> Result<Ordering, Error>,
    ) -> Result<u64, Error> {
        let map = self.maps.get_mut(map).ok_or(Error::Unauthorized)?;
        let (path, found) = Node::search(map, compare)?;
        let mut slot = &mut *map;
        for &side in &path {
            slot = &mut slot.as_mut().unwrap().children[side];
        }
        if slot.as_ref().map_or(0, |node| node.version) != expected {
            return Err(Error::Empty);
        }
        let version = self.version.checked_add(1).ok_or(Error::Full)?;
        if found {
            let node = slot.as_mut().unwrap();
            node.version = version;
            node.value = value;
        } else {
            Node::insert(
                map,
                &path,
                Box::new(Node {
                    key,
                    version,
                    value,
                    children: [None, None],
                    height: 1,
                }),
            );
        }
        self.version = version;
        Ok(version)
    }
    pub fn remove(
        &mut self,
        map: usize,
        compare: impl FnMut(&K) -> Result<Ordering, Error>,
    ) -> Result<(K, u64, V), Error> {
        let map = self.maps.get_mut(map).ok_or(Error::Unauthorized)?;
        let (path, found) = Node::search(map, compare)?;
        if !found {
            return Err(Error::Empty);
        }
        let node = Node::remove(map, &path);
        Ok((node.key, node.version, node.value))
    }
}
