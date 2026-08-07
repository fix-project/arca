extern crate alloc;
use crate::scheduler::{Scheduler, Task};
use alloc::sync::Arc;
use kernel::{coreid, kthread};

use crate::handle::*;
use crate::runtime::Runtime;
use crate::storage::Storage;
use kernel::prelude::*;

const NUM_WORKERS: usize = 19;
#[derive(Clone, Copy)]
enum EvalType {
    Parallel,
    Serial,
}
pub struct Evaluator<R: Runtime> {
    runtime: R,
    scheduler: Scheduler,
}

impl<R: Runtime> Evaluator<R> {
    pub fn new(runtime: R) -> Arc<Self> {
        let evaluator = Arc::new(Self {
            runtime,
            scheduler: Scheduler::new(),
        });

        evaluator.start_workers(NUM_WORKERS);
        evaluator
    }

    pub fn start_workers(self: &Arc<Self>, num_workers: usize) {
        for i in 0..num_workers {
            let evaluator = Arc::clone(self);
            kthread::spawn(move || {
                println!("worker {} started on core {}", i, coreid());
                evaluator.worker_loop(i);
            })
        }
    }

    pub fn worker_loop(self: &Arc<Self>, _worker_id: usize) {
        loop {
            // eventually make this a queue to handle local queue of work
            if let Some(work) = self.scheduler.get_work() {
                //println!("worker {} evaluating task on core {}", worker_id, coreid());

                let result = self.eval_test(work.get_handle(), EvalType::Parallel);
                work.task_complete(result)
            } else {
                //change this to condition variable, so it does no busy waiting?
                kthread::yield_now()
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

                let result = self.eval_test(work.get_handle(), EvalType::Parallel);

                work.task_complete(result);
            } else {
                // Workers already claimed all available tasks.
                kthread::yield_now();
            }
        }
    }

    pub fn storage(&self) -> &dyn Storage {
        self.runtime.storage()
    }

    fn apply(&self, combination: Tree) -> Handle {
        self.runtime.execute(combination)
    }

    pub fn lift(&self, handle: Handle) -> Handle {
        match handle {
            Handle::Ref(r) => match r {
                Ref::Tree(t) => Object::Tree(t).into(),
                Ref::Blob(b) => Object::Blob(b).into(),
            },
            _ => handle,
        }
    }

    pub fn lower(&self, handle: Handle) -> Handle {
        match handle {
            Handle::Object(r) => match r {
                Object::Tree(t) => Ref::Tree(t).into(),
                Object::Blob(b) => Ref::Blob(b).into(),
            },
            _ => handle,
        }
    }

    fn think(&self, thunk: Thunk, eval_mode: EvalType) -> Handle {
        match thunk {
            Thunk::Identification(reference) => self.lift(Handle::Ref(reference)),
            Thunk::Selection(_) => todo!(),
            Thunk::Application(tree) => {
                let evaled = self.eval_tree(tree, eval_mode);
                self.apply(evaled)
            }
        }
    }

    fn force(&self, thunk: Thunk, eval_mode: EvalType) -> Handle {
        let thought = self.think(thunk, eval_mode);
        match thought {
            Handle::Object(_) => thought,
            Handle::Ref(_) => self.lift(thought),
            Handle::Thunk(_) | Handle::Encode(_) => todo!(),
        }
    }

    fn encode(&self, encode: Encode, eval_mode: EvalType) -> Handle {
        match encode {
            Encode::Strict(thunk) => self.lift(self.force(thunk, eval_mode)),
            Encode::Shallow(thunk) => self.lower(self.force(thunk, eval_mode)),
        }
    }

    fn eval_tree(&self, handle: Tree, eval_mode: EvalType) -> Tree {
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
        self.runtime.storage().add_tree(&evaled)
    }

    fn eval_tree_parallel(&self, handle: Tree) -> Tree {
        let tree = self.runtime.storage().get_tree(handle).unwrap();
        if tree.len() <= 3 {
            return self.eval_tree_seq(handle);
        }
        let mut evaled = Vec::with_capacity(tree.len());
        // evaluate the first
        evaled.push(self.eval_test(tree[0], EvalType::Serial));
        let mut tasks = Vec::with_capacity(tree.len() - 1);
        for child in tree[1..].iter().copied() {
            tasks.push(self.scheduler.push_work(child));
        }
        for task in tasks {
            evaled.push(self.wait_while_helping(&task));
        }
        self.runtime.storage().add_tree(&evaled)
    }
    // elimnate redundancy i think
    fn eval_test(&self, handle: Handle, eval_mode: EvalType) -> Handle {
        //println!("evaluating {handle}");
        match handle {
            Handle::Ref(reference) => self.eval(self.lift(Handle::Ref(reference))),
            Handle::Thunk(_) => todo!(),
            Handle::Object(obj) => match obj {
                Object::Blob(blob) => blob.into(),
                Object::Tree(tree) => self.eval_tree(tree, eval_mode).into(),
            },
            Handle::Encode(e) => self.eval_test(self.encode(e, eval_mode), eval_mode),
        }
    }

    pub fn eval(&self, handle: Handle) -> Handle {
        self.eval_test(handle, EvalType::Parallel)
    }
}
