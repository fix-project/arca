extern crate alloc;
use crate::scheduler::{Scheduler, Task};
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use hashbrown::HashMap;
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
    trees: kthread::KMutex<HashMap<[u8; HANDLE_SIZE], Tree>>,
    stopped: AtomicBool,
    workers: AtomicUsize,
}

impl<R: Runtime + Send + Sync + 'static> Evaluator<R> {
    pub fn new(runtime: R, parallelism: usize) -> Arc<Self> {
        let evaluator = Arc::new(Self {
            runtime,
            scheduler: Scheduler::new(),
            trees: kthread::KMutex::new(HashMap::new()),
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
        self.runtime.execute(combination)
    }

    pub fn select(&self, selection: Tree) -> Handle {
        self.select_with_mode(selection, EvalType::Parallel)
    }

    fn select_with_mode(&self, selection: Tree, eval_mode: EvalType) -> Handle {
        let handles = self.storage().get_tree(selection).unwrap();
        let (&target, indices) = handles.split_first().expect("empty selection thunk");
        let target = match target.view() {
            HandleView::Encode(encode) => self.encode(*encode, eval_mode),
            _ => target,
        };
        match indices {
            [index] => self.select_index(target, self.read_index_with_mode(*index, eval_mode)),
            [start, end] => self.select_range(
                target,
                self.read_index_with_mode(*start, eval_mode),
                self.read_index_with_mode(*end, eval_mode),
            ),
            _ => panic!("selection thunk got {} handles", handles.len()),
        }
    }

    pub fn select_index(&self, target: Handle, index: usize) -> Handle {
        if index >= target.len() {
            panic!("Invalid index {index} for selection thunk");
        }
        let object = match target.view() {
            HandleView::Object(object) => *object,
            HandleView::Ref(reference) => reference.object_descriptor(),
            _ => panic!("expected blob or tree handle for selection thunk"),
        };
        match object.view() {
            ObjectView::Tree(tree) => self.storage().get_tree(*tree).unwrap()[index],
            ObjectView::Blob(blob) => {
                let data = self.storage().get_blob(*blob).unwrap();
                self.storage().add_blob(&[data[index]]).into_ref().into()
            }
        }
    }

    pub fn select_range(&self, target: Handle, begin: usize, end: usize) -> Handle {
        if begin >= end {
            panic!("Invalid range [{begin}, {end}) for seleciton thunk");
        }
        let object = match target.view() {
            HandleView::Object(object) => *object,
            HandleView::Ref(reference) => reference.object_descriptor(),
            _ => panic!("expected blob or tree handle for selection thunk"),
        };
        match object.view() {
            ObjectView::Tree(tree) => {
                let data = self.storage().get_tree(*tree).unwrap();
                self.storage().add_tree(&data[begin..end]).into_ref().into()
            }
            ObjectView::Blob(blob) => {
                let data = self.storage().get_blob(*blob).unwrap();
                self.storage().add_blob(&data[begin..end]).into_ref().into()
            }
        }
    }

    pub fn read_index(&self, handle: Handle) -> usize {
        self.read_index_with_mode(handle, EvalType::Parallel)
    }

    fn read_index_with_mode(&self, handle: Handle, eval_mode: EvalType) -> usize {
        let blob = fixhandle::Blob::try_from(self.eval_test(handle, eval_mode))
            .expect("expected blob handle for selection index");
        let bytes = self.storage().get_blob(blob).unwrap();
        // Make buffer fit all supported integer widths
        let mut buffer = [0; 16];
        assert!(bytes.len() <= buffer.len());
        buffer[..bytes.len()].copy_from_slice(&bytes);
        usize::try_from(u128::from_le_bytes(buffer)).expect("selection index should be in range")
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
            ThunkView::Identification(reference) => self.lift(reference.into()),
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
            HandleView::Object(_) => thought,
            HandleView::Ref(_) => self.lift(thought),
            HandleView::Thunk(thunk) => self.force(*thunk, eval_mode),
            HandleView::Encode(encode) => self.lift(self.encode(*encode, eval_mode)),
        }
    }

    fn encode(&self, encode: Encode, eval_mode: EvalType) -> Handle {
        match encode.view() {
            EncodeView::Strict(thunk) => self.eval_test(self.force(thunk, eval_mode), eval_mode),
            EncodeView::Shallow(thunk) => self.lower(self.force(thunk, eval_mode)),
        }
    }

    fn eval_tree(&self, handle: Tree, eval_mode: EvalType) -> Tree {
        if handle.as_handle().is_eq()
            && let Some(result) = self
                .trees
                .lock()
                .get(handle.as_handle().as_bytes())
                .copied()
        {
            return result;
        }
        let result = match eval_mode {
            EvalType::Serial => self.eval_tree_seq(handle),
            EvalType::Parallel => self.eval_tree_parallel(handle),
        };
        if handle.as_handle().is_eq() {
            self.trees
                .lock()
                .insert(*handle.as_handle().as_bytes(), result);
        }
        result
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
            HandleView::Ref(reference) => self.eval_test(self.lift((*reference).into()), eval_mode),
            HandleView::Thunk(_) => handle,
            HandleView::Object(obj) => match obj.view() {
                ObjectView::Blob(blob) => (*blob).into(),
                ObjectView::Tree(tree) => self.eval_tree(*tree, eval_mode).into(),
            },
            HandleView::Encode(e) => self.encode(*e, eval_mode),
        }
    }

    pub fn eval(&self, handle: Handle) -> Handle {
        self.eval_test(handle, EvalType::Parallel)
    }
}
