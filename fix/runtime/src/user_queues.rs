// file includes how the queues are created functions that it can call on it
use alloc::{collections::VecDeque, vec::Vec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Empty,
    Full,
    Closed,
    Unauthorized,
    Invalid,
}

/// All operations require external synchronization.
pub struct WorkQueues<T: Copy> {
    queues: Vec<VecDeque<T>>,
    capacities: Vec<usize>,
    grants: Vec<Claims<T>>,
    closed: Vec<bool>,
    max_capacity: usize,
    max_queues: usize,
    max_held: usize,
}

impl<T: Copy> WorkQueues<T> {
    
}