// Shared state for one scheduling domain. The host and kernel use the same mechanism.
// Callers serialize these short methods, then run returned actions outside the lock.
// Contains the task graph and work queues/grant tables. Connects queue operations to 
// task execution, manages published queue IDs, and tracks cancellation and execution counts.
extern crate alloc;
use crate::task_graph::{TaskGraph, Transition};
use crate::work_queues::{Error, WorkQueues};
use alloc::{vec, vec::Vec};

pub struct Domain<A, V> {
    queues: WorkQueues<usize>,
    graph: TaskGraph<A, V>,
    published: Vec<Option<u32>>,
    pub aborted: Option<&'static str>,
    pub executed: Vec<usize>,
    pub steals: usize,
    pub peak_in_flight: usize,
}
impl<A, V> Domain<A, V> {
    pub fn new(workers: usize, root: A) -> Self {
        Self {
            queues: WorkQueues::new(workers, 65536),
            graph: TaskGraph::new(root),
            published: vec![None; workers],
            aborted: None,
            executed: vec![0; workers],
            steals: 0,
            peak_in_flight: 0,
        }
    }
    pub fn create(&mut self, id: usize, capacity: usize) -> Result<u32, Error> {
        self.queues.create(id, capacity)
    }
    pub fn publish(&mut self, id: usize, queue: usize) -> Result<(), Error> {
        self.queues.len(id, queue)?;
        self.published[id] = Some(queue as u32);
        Ok(())
    }
    pub fn peer(&self, id: usize) -> Option<u32> {
        self.published.get(id).copied().flatten()
    }
    pub fn refill(&mut self, id: usize, queue: usize, limit: u32) -> Result<usize, Error> {
        self.queues.len(id, queue)?;
        if limit == 0 || limit > 65536 {
            return Err(Error::Invalid);
        }
        let mut count = 0;
        while count < limit as usize {
            let Some(task) = self.graph.next_ready() else {
                break;
            };
            if let Err(e) = self.queues.seed(queue, task) {
                self.graph.put_ready_back(task);
                if e == Error::Full && count > 0 {
                    break;
                }
                return Err(e);
            }
            count += 1;
        }
        if count == 0 {
            Err(Error::Empty)
        } else {
            Ok(count)
        }
    }
    pub fn pop(&mut self, id: usize, queue: usize) -> Result<u64, Error> {
        self.queues.pop(id, queue)
    }
    pub fn push(&mut self, id: usize, queue: usize, slot: u64) -> Result<(), Error> {
        self.queues.push(id, queue, slot)
    }
    pub fn steal(
        &mut self,
        id: usize,
        source: usize,
        destination: usize,
        count: u32,
    ) -> Result<usize, Error> {
        let n = self.queues.steal(id, source, destination, count)?;
        self.steals += 1;
        Ok(n)
    }
    pub fn len(&self, id: usize, queue: usize) -> Result<usize, Error> {
        self.queues.len(id, queue)
    }
    pub fn start(&mut self, id: usize, slot: u64) -> Result<(usize, A, Vec<V>), Error> {
        if self.aborted.is_some() {
            return Err(Error::Closed);
        }
        let task = self.queues.consume(id, slot)?;
        let (action, inputs) = self.graph.start(task).ok_or(Error::Invalid)?;
        self.peak_in_flight = self.peak_in_flight.max(self.graph.in_flight());
        Ok((task, action, inputs))
    }
    pub fn finish(&mut self, worker: usize, task: usize, result: Transition<A, V>) {
        self.graph.finish(task, result);
        self.executed[worker] += 1;
    }
    pub fn close(&mut self, id: usize) {
        if let Ok(tasks) = self.queues.close(id) {
            for task in tasks {
                self.graph.put_ready_back(task);
            }
        }
        // Published queues outlive their creator. Keep discovery stable so a late
        // starter cannot miss a peer that already finished a short workload.
    }
    pub fn done(&self) -> bool {
        self.graph.result().is_some()
    }
    pub fn result(&self) -> Option<&V> {
        self.graph.result()
    }
    pub fn live(&self) -> usize {
        self.graph.live()
    }
}
