// Trusted shared-domain queue mechanism. All methods require external synchronization.
extern crate alloc;
use alloc::{collections::VecDeque, vec::Vec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Empty,
    Full,
    Closed,
    Unauthorized,
    Invalid,
}

#[derive(Clone, Copy)]
struct Grant<T> {
    generation: u32,
    value: Option<T>,
}

struct Claims<T> {
    entries: Vec<Grant<T>>,
    free: Vec<usize>,
}

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
    // Creates a domain with one grant table per worker and no queues yet.
    // Defaults to at least 64 allowed queues and 1024 grant slots per worker.
    pub fn new(workers: usize, max_capacity: usize) -> Self {
        Self::with_limits(workers, 64.max(workers * 4), max_capacity, 1024)
    }

    // Initializes the workers and sets limits on queue count, queue capacity,
    // and the number of grant slots each worker can allocate.
    pub fn with_limits(
        workers: usize,
        max_queues: usize,
        max_capacity: usize,
        max_held: usize,
    ) -> Self {
        assert!(workers > 0 && max_held > 0 && max_held < u32::MAX as usize);
        Self {
            queues: Vec::new(),
            capacities: Vec::new(),
            grants: (0..workers)
                .map(|_| Claims {
                    entries: Vec::new(),
                    free: Vec::new(),
                })
                .collect(),
            closed: alloc::vec![false; workers],
            max_capacity,
            max_queues,
            max_held,
        }
    }

    // Checks that the worker belongs to this domain and has not been closed.
    fn member(&self, worker: usize) -> Result<(), Error> {
        match self.closed.get(worker) {
            None => Err(Error::Unauthorized),
            Some(true) => Err(Error::Closed),
            Some(false) => Ok(()),
        }
    }

    // Creates an empty queue with the requested capacity and returns its ID.
    // Any active worker in the domain can access it, not just its creator.
    pub fn create(&mut self, worker: usize, capacity: usize) -> Result<u32, Error> {
        self.member(worker)?;
        if capacity == 0 || capacity > self.max_capacity {
            return Err(Error::Invalid);
        }
        if self.queues.len() >= self.max_queues {
            return Err(Error::Full);
        }
        let id = self.queues.len() as u32;
        self.queues.push(VecDeque::new());
        self.capacities.push(capacity);
        Ok(id)
    }

    // Checks that the queue ID refers to an existing queue in this domain.
    fn queue(&self, queue: usize) -> Result<(), Error> {
        if queue >= self.queues.len() {
            Err(Error::Unauthorized)
        } else {
            Ok(())
        }
    }

    // Adds a work item to the back of a queue if there is space.
    // Used by trusted code; this does not require a worker's grant.
    pub fn seed(&mut self, queue: usize, value: T) -> Result<(), Error> {
        self.queue(queue)?;
        if self.queues[queue].len() >= self.capacities[queue] {
            return Err(Error::Full);
        }
        self.queues[queue].push_back(value);
        Ok(())
    }

    // Looks up a token in this worker's grant table and returns its slot and work item.
    // The token contains the generation and slot index plus one.
    // Rejects stale or empty entries without consuming a valid grant.
    fn resolve(&self, worker: usize, token: u64) -> Result<(usize, T), Error> {
        self.member(worker)?;
        let index = (token as u32).checked_sub(1).ok_or(Error::Unauthorized)? as usize;
        let grant = self.grants[worker]
            .entries
            .get(index)
            .ok_or(Error::Unauthorized)?;
        if grant.generation != (token >> 32) as u32 {
            return Err(Error::Unauthorized);
        }
        Ok((index, grant.value.ok_or(Error::Unauthorized)?))
    }

    // Takes the newest item from the queue and records it in the worker's grant table.
    // Returns a token that the worker can use to execute or requeue the item.
    // Leaves the queue unchanged if no grant slot is available.
    pub fn pop(&mut self, worker: usize, queue: usize) -> Result<u64, Error> {
        self.member(worker)?;
        self.queue(queue)?;
        if self.queues[queue].is_empty() {
            return Err(Error::Empty);
        }
        let claims = &mut self.grants[worker];
        let index = if let Some(i) = claims.free.pop() {
            i
        } else {
            if claims.entries.len() >= self.max_held {
                return Err(Error::Full);
            }
            claims.entries.push(Grant {
                generation: 1,
                value: None,
            });
            claims.entries.len() - 1
        };
        let grant = &mut claims.entries[index];
        grant.value = self.queues[queue].pop_back();
        Ok(((grant.generation as u64) << 32) | (index as u64 + 1))
    }

    // Clears a grant entry and advances its generation so old tokens become invalid.
    // Makes the slot available for reuse unless its generation counter is exhausted.
    fn revoke(&mut self, worker: usize, index: usize) {
        let claims = &mut self.grants[worker];
        let grant = &mut claims.entries[index];
        grant.value = None;
        // Retire an exhausted entry instead of wrapping and reviving stale authority.
        if let Some(next) = grant.generation.checked_add(1) {
            grant.generation = next;
            claims.free.push(index);
        }
    }

    // Moves a worker's held item to the back of the target queue.
    // Revokes the grant on success; a failed push leaves it valid for retry.
    pub fn push(&mut self, worker: usize, target: usize, token: u64) -> Result<(), Error> {
        let (index, value) = self.resolve(worker, token)?;
        self.seed(target, value)?;
        self.revoke(worker, index);
        Ok(())
    }

    // Moves up to max_tasks of the oldest source items to the back of the destination.
    // Returns how many moved. If the selected batch will not fit, moves nothing.
    // Transfers work directly between queues without creating grants.
    pub fn steal(
        &mut self,
        worker: usize,
        source: usize,
        destination: usize,
        max_tasks: u32,
    ) -> Result<usize, Error> {
        self.member(worker)?;
        self.queue(source)?;
        self.queue(destination)?;
        if source == destination {
            return Err(Error::Invalid);
        }
        if max_tasks == 0 || max_tasks as usize > self.max_capacity {
            return Err(Error::Invalid);
        }
        let count = (max_tasks as usize).min(self.queues[source].len());
        if count == 0 {
            return Err(Error::Empty);
        }
        if count > self.capacities[destination] - self.queues[destination].len() {
            return Err(Error::Full);
        }
        for _ in 0..count {
            let value = self.queues[source].pop_front().unwrap();
            self.queues[destination].push_back(value);
        }
        Ok(count)
    }

    // Takes the work item associated with a valid grant and revokes the grant.
    // Returns the item to the caller; this function does not execute it.
    pub fn consume(&mut self, worker: usize, token: u64) -> Result<T, Error> {
        let (index, value) = self.resolve(worker, token)?;
        self.revoke(worker, index);
        Ok(value)
    }

    // Returns the number of items currently in the queue.
    // Does not count work already popped into grant tables or running.
    pub fn len(&self, worker: usize, queue: usize) -> Result<usize, Error> {
        self.member(worker)?;
        self.queue(queue)?;
        Ok(self.queues[queue].len())
    }

    // Closes the worker and returns any work it still holds in its grant table.
    // The caller can requeue that work. Shared queues remain accessible to other workers.
    pub fn close(&mut self, worker: usize) -> Result<Vec<T>, Error> {
        self.member(worker)?;
        let mut recovered = Vec::new();
        for grant in &mut self.grants[worker].entries {
            if let Some(value) = grant.value.take() {
                recovered.push(value);
            }
        }
        self.closed[worker] = true;
        Ok(recovered)
    }
}