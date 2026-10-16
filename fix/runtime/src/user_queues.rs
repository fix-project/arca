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

/// create a vector of queues (empty)
pub struct WorkQueues<T: Copy> {
    queues: Vec<VecDeque<T>>,
    capacities: Vec<usize>,
    closed: Vec<bool>,
    max_capacity: usize,
    max_queues: usize,
}

impl<T: Copy> WorkQueues<T> {
    pub fn new(workers: usize, max_capacity: usize, max_queues: usize) -> Self {
        assert!(workers > 0);

        Self {
            queues: Vec::new(),
            capacities: Vec::new(),
            closed: alloc::vec![false; workers],
            max_capacity,
            max_queues,
        }
    }

    fn member(&self, worker: usize) -> Result<(), Error> {
        match self.closed.get(worker) {
            None => Err(Error::Unauthorized),
            Some(true) => Err(Error::Closed),
            Some(false) => Ok(()),
        }
    }

    pub fn create_queue(&mut self, worker: usize, capacity: usize) -> Result<u32, Error> {
        self.member(worker)?;

        if capacity == 0 || capacity > self.max_capacity {return Err(Error::Invalid);}
        if self.queues.len() >= self.max_queues {return Err(Error::Full);}
        
        let id = u32::try_from(self.queues.len()).map_err(|_| Error::Full)?;

        self.queues.push(VecDeque::new());
        self.capacities.push(capacity);

        Ok(id)
    }

    fn queue(&self, queue: usize) -> Result<(), Error> {
        if queue >= self.queues.len() {
            Err(Error::Unauthorized)
        } else {
            Ok(())
        }
    }

    pub fn push(&mut self, worker: usize, queue: usize, value: T) -> Result<(), Error> {
        self.member(worker)?;
        self.queue(queue)?;
        if self.queues[queue].len() >= self.capacities[queue] {
            return Err(Error::Full);
        }
        self.queues[queue].push_back(value);
        Ok(())
    }

    pub fn pop(&mut self, worker: usize, queue: usize) -> Result<T, Error> {
        self.member(worker)?;
        self.queue(queue)?;

        self.queues[queue].pop_front().ok_or(Error::Empty)
    }
    
    pub fn len(&self, worker: usize, queue: usize) -> Result<usize, Error> {
        self.member(worker)?;
        self.queue(queue)?;

        Ok(self.queues[queue].len())
    }

    pub fn queue_count(&self, worker: usize) -> Result<usize, Error> {
        self.member(worker)?;
        Ok(self.queues.len())
    }

    pub fn close(&mut self, worker: usize) -> Result<(), Error> {
        self.member(worker)?;
        self.closed[worker] = true;
        Ok(())
    }
}   