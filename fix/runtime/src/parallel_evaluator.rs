extern crate alloc;
use crate::scheduler::{Scheduler, Task};
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[cfg(debug_assertions)]
use kernel::coreid;
use kernel::kthread;

use crate::handle::*;
use crate::runtime::Runtime;
use crate::storage::Storage;
use kernel::prelude::*;

#[derive(Clone, Copy)]
enum EvalType {
    Parallel,
    Serial,
}
pub struct Evaluator<R: Runtime> {
    runtime: R,
    scheduler: Scheduler,
    stopped: AtomicBool,
    workers: AtomicUsize,
}

impl<R: Runtime + Send + Sync + 'static> Evaluator<R> {
    pub fn new(runtime: R, parallelism: usize) -> Arc<Self> {
        let evaluator = Arc::new(Self {
            runtime,
            scheduler: Scheduler::new(),
            stopped: AtomicBool::new(false),
            workers: AtomicUsize::new(0),
        });

        evaluator.start_workers(parallelism.saturating_sub(1));
        evaluator
    }

    pub fn start_workers(self: &Arc<Self>, num_workers: usize) {
        for i in 0..num_workers {
            self.workers.fetch_add(1, Ordering::Relaxed);
            let evaluator = Arc::clone(self);
            kthread::spawn(move || {
                #[cfg(debug_assertions)]
                println!("worker {} started on core {}", i, coreid());
                evaluator.worker_loop(i);
                evaluator.workers.fetch_sub(1, Ordering::Release);
            })
        }
    }

    /// Call after evaluation finishes to stop and release the worker threads.
    pub fn shutdown(&self) {
        self.stopped.store(true, Ordering::Release);
        self.scheduler.wake_workers();
        while self.workers.load(Ordering::Acquire) != 0 {
            kthread::yield_now();
        }
    }

    pub fn worker_loop(self: &Arc<Self>, _worker_id: usize) {
        while !self.stopped.load(Ordering::Acquire) {
            // eventually make this a queue to handle local queue of work
            if let Some(work) = self
                .scheduler
                .wait_for_work(|| self.stopped.load(Ordering::Acquire))
            {
                //println!("worker {} evaluating task on core {}", worker_id, coreid());

                let result = self.eval_test(work.get_handle(), EvalType::Parallel);
                work.task_complete(result)
            }
        }
    }

    fn wait_while_helping(&self, target: &Arc<Task>) -> Handle {
        loop {
            // The task we are waiting for has completed.
            if target.is_complete() {
                return target.take_result();
            }

            if let Some(work) = self.scheduler.get_work() {
                //println!("calling thread helping on core {}", coreid());

                let result = self.eval_test(work.get_handle(), EvalType::Serial);

                work.task_complete(result);
            } else {
                return target.take_result();
            }
        }
    }

    pub fn storage(&self) -> &dyn Storage {
        self.runtime.storage()
    }

    fn apply(&self, combination: Tree) -> Handle {
        let coupon = self.runtime.execute(combination);
        let claim = self.storage().certificate_claim(coupon).unwrap();
        self.storage().get_tree(claim).unwrap()[2]
    }

    pub fn select(&self, selection: Tree) -> Handle {
        self.select_with_mode(selection, EvalType::Parallel)
    }

    fn select_with_mode(&self, selection: Tree, eval_mode: EvalType) -> Handle {
        let selection = self.eval_tree(selection, eval_mode);
        self.storage().select(selection).expect("invalid selection")
    }

    pub fn lift(&self, handle: Handle) -> Handle {
        match handle.view() {
            HandleView::Ref(reference) => reference.object_descriptor().into(),
            _ => handle,
        }
    }

    pub fn lower(&self, handle: Handle) -> Handle {
        match handle.view() {
            HandleView::Object(object) => object.into_ref().into(),
            _ => handle,
        }
    }

    fn think(&self, thunk: Thunk, eval_mode: EvalType) -> Handle {
        match thunk.view() {
            ThunkView::Identification(reference) => reference.into(),
            ThunkView::Selection(tree) => {
                self.select_with_mode(tree.object_descriptor(), eval_mode)
            }
            ThunkView::Application(tree) => {
                let evaled = self.eval_tree(tree.object_descriptor(), eval_mode);
                self.apply(evaled)
            }
        }
    }

    fn force(&self, thunk: Thunk, eval_mode: EvalType) -> Handle {
        let thought = self.think(thunk, eval_mode);
        match thought.view() {
            HandleView::Object(_) | HandleView::Ref(_) => thought,
            HandleView::Thunk(thunk) => self.force(*thunk, eval_mode),
            HandleView::Encode(encode) => match encode.view() {
                EncodeView::Strict(thunk) | EncodeView::Shallow(thunk) => {
                    self.force(thunk, eval_mode)
                }
            },
        }
    }

    fn encode(&self, encode: Encode, eval_mode: EvalType) -> Handle {
        match encode.view() {
            EncodeView::Strict(thunk) => self.lift(self.force(thunk, eval_mode)),
            EncodeView::Shallow(thunk) => self.lower(self.force(thunk, eval_mode)),
        }
    }

    fn eval_tree(&self, handle: Tree, eval_mode: EvalType) -> Tree {
        if handle.as_handle().is_value() {
            return handle;
        }
        match eval_mode {
            EvalType::Serial => self.eval_tree_seq(handle),
            EvalType::Parallel => self.eval_tree_parallel(handle),
        }
    }

    fn eval_tree_seq(&self, handle: Tree) -> Tree {
        let tree = self.runtime.storage().get_tree(handle).unwrap();
        let evaled: Vec<Handle> = tree
            .as_ref()
            .iter()
            .copied()
            .map(|x| self.eval_test(x, EvalType::Serial))
            .collect();
        if tree
            .iter()
            .zip(&evaled)
            .all(|(a, b)| a.as_bytes() == b.as_bytes())
        {
            return handle;
        }
        let result = self.runtime.storage().add_tree(&evaled);
        if handle.is_tag() {
            result.tag_descriptor()
        } else {
            result
        }
    }

    fn eval_tree_parallel(&self, handle: Tree) -> Tree {
        let tree = self.runtime.storage().get_tree(handle).unwrap();
        let mut evaled = Vec::with_capacity(tree.len());
        let mut tasks = Vec::with_capacity(tree.len());
        for child in tree.iter().copied() {
            tasks.push(self.scheduler.push_work(child));
        }
        for task in tasks {
            evaled.push(self.wait_while_helping(&task));
        }
        if tree
            .iter()
            .zip(&evaled)
            .all(|(a, b)| a.as_bytes() == b.as_bytes())
        {
            return handle;
        }
        let result = self.runtime.storage().add_tree(&evaled);
        if handle.is_tag() {
            result.tag_descriptor()
        } else {
            result
        }
    }
    // elimnate redundancy i think
    fn eval_test(&self, handle: Handle, eval_mode: EvalType) -> Handle {
        //println!("evaluating {handle}");
        match handle.view() {
            HandleView::Ref(_) | HandleView::Thunk(_) => handle,
            HandleView::Object(obj) => match obj.view() {
                ObjectView::Blob(blob) => (*blob).into(),
                ObjectView::Tree(tree) => self.eval_tree(*tree, eval_mode).into(),
            },
            HandleView::Encode(e) => self.eval_test(self.encode(*e, eval_mode), eval_mode),
        }
    }

    pub fn eval(&self, handle: Handle) -> Handle {
        self.eval_test(handle, EvalType::Parallel)
    }
}
