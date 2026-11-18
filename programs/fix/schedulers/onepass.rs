use crate::parallel::Parallel;
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec,
    vec::Vec,
};
use core::cell::RefCell;
use core::{
    future::Future,
    task::{Context, Poll, Waker},
};
use fixscheduler::collector;
use fixscheduler::executor::Graph;
use fixscheduler::{
    executor::{
        Scheduler,
        graph::{Claim, Node},
    },
    kernel::Advice,
    remote::{Cluster, protocol::Information},
    shell::{AsValue, DataType, Value, ValueType, child},
    storage,
};
use fixwasm::{Error, ThunkType, tree};

pub struct OnePass<'a> {
    graph: &'a Graph,
    parallel: Parallel<'a>,
    advice: RefCell<BTreeMap<[u8; 32], u64>>,
    sketch: RefCell<Option<BTreeMap<Node, Job>>>,
}

impl Scheduler for OnePass<'_> {
    /// Resolve eval using this scheduler's evaluation and placement choices.
    async fn eval(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::eval(input)?;
        if self.graph.current().is_none() {
            self.schedule(&node, || async {
                Parallel::eval_using(self, input, &mut |coupon| self.parallel.retain(coupon)).await
            })
            .await
        } else {
            self.get(&node, || async {
                Parallel::eval_using(self, input, &mut |coupon| self.parallel.retain(coupon)).await
            })
            .await
        }
    }
    /// Resolve think using this scheduler's evaluation and placement choices.
    async fn think(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::think(input)?;
        if self.graph.current().is_none() {
            self.schedule(&node, || async {
                collector::think(self, input, &mut |coupon| self.parallel.retain(coupon)).await
            })
            .await
        } else {
            self.get(&node, || async {
                collector::think(self, input, &mut |coupon| self.parallel.retain(coupon)).await
            })
            .await
        }
    }
    /// Resolve force using this scheduler's evaluation and placement choices.
    async fn force(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::force(input)?;
        if self.graph.current().is_none() {
            self.schedule(&node, || async {
                collector::force(self, input, &mut |coupon| self.parallel.retain(coupon)).await
            })
            .await
        } else {
            self.get(&node, || async {
                collector::force(self, input, &mut |coupon| self.parallel.retain(coupon)).await
            })
            .await
        }
    }
    /// Resolve apply using this scheduler's evaluation and placement choices.
    async fn apply(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::apply(input)?;
        if self.graph.current().is_none() {
            self.schedule(&node, || async {
                collector::apply(self, input, &mut |coupon| self.parallel.retain(coupon)).await
            })
            .await
        } else {
            self.get(&node, || async {
                collector::apply(self, input, &mut |coupon| self.parallel.retain(coupon)).await
            })
            .await
        }
    }
    /// Resolve select using this scheduler's evaluation and placement choices.
    async fn select(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::select(input)?;
        if self.graph.current().is_none() {
            self.schedule(&node, || async {
                collector::select(self, input, &mut |coupon| self.parallel.retain(coupon)).await
            })
            .await
        } else {
            self.get(&node, || async {
                collector::select(self, input, &mut |coupon| self.parallel.retain(coupon)).await
            })
            .await
        }
    }

    /// Inspect local data during discovery; use Parallel's acquisition when executing.
    async fn load(&self, value: &Value) -> Result<Value, Error> {
        if self.sketch.borrow().is_some() {
            match storage::identify(value)? {
                Some(value) => Ok(value),
                None => core::future::pending().await,
            }
        } else {
            self.parallel.load(value).await
        }
    }

    /// Extend Parallel's advice handling with output-size and parallelism estimates.
    fn on_advice(&self, advice: &Advice) -> Result<(), Error> {
        self.parallel.on_advice(advice)?;
        if !(advice.key_is("fix::output-size")? || advice.key_is("fix::parallel")?)
            || unsigned(&advice.payload)?.is_none()
        {
            return Ok(());
        }
        let target = if matches!(
            advice.target.value_type(),
            ValueType::Thunk | ValueType::Ref | ValueType::Encode
        ) {
            storage::identify(&advice.target)?
        } else {
            Some(advice.target.clone())
        };
        if let Some(target) = target {
            let key = storage::canonical_value(&fixscheduler::shell::capture(tree![
                target,
                &advice.key
            ])?)?;
            self.advice
                .borrow_mut()
                .insert(key.raw_handle_bytes()?, advice.payload.read_u64()?);
        }
        Ok(())
    }
}

/// Locality and output estimates for one request in the current scheduling round.
#[derive(Default)]
pub struct Job {
    pub dependencies: BTreeSet<Node>,
    pub present: BTreeMap<usize, u64>,
    pub cached: BTreeSet<usize>,
    pub output_bytes: u64,
    pub memory_bytes: Option<u64>,
    pub parallel: bool,
}

fn unsigned(value: &Value) -> Result<Option<u64>, Error> {
    if value.value_type() == ValueType::Object
        && value.data_type()? == DataType::Blob
        && value.len()? == 8
    {
        value.read_u64().map(Some)
    } else {
        Ok(None)
    }
}

pub fn eligible(job: &Job, worker: &Information) -> bool {
    worker.parallelism > 0
        && match (job.memory_bytes, worker.memory_bytes) {
            (Some(required), Some(available)) => required <= available,
            _ => true,
        }
}

/// Assign dependencies before parents, preferring cached results and then resident bytes.
pub fn assign(
    root: &Node,
    jobs: &BTreeMap<Node, Job>,
    workers: &[Information],
    assignments: &mut BTreeMap<Node, (usize, u64)>,
) {
    let mut pending = vec![(root.clone(), false)];
    let mut seen = BTreeSet::new();
    while let Some((request, visited)) = pending.pop() {
        let job = &jobs[&request];
        if !visited {
            if seen.insert(request.clone()) {
                pending.push((request, true));
                pending.extend(
                    job.dependencies
                        .iter()
                        .rev()
                        .map(|child| (child.clone(), false)),
                );
            }
            continue;
        }
        if !(request.is_eval() || request.is_think() || request.is_select()) {
            continue;
        }
        if let Some(worker) = job
            .cached
            .iter()
            .find(|worker| workers[**worker].parallelism > 0)
        {
            assignments.insert(request, (*worker, 0));
            continue;
        }
        if job.parallel && eligible(job, &workers[0]) {
            assignments.insert(request, (0, 0));
            continue;
        }
        let mut present = job.present.clone();
        for child in &job.dependencies {
            let output = jobs[child].output_bytes;
            if let Some(&(worker, _)) = assignments.get(child) {
                let bytes = present.entry(worker).or_default();
                *bytes = bytes.saturating_add(output);
            } else {
                for &worker in &jobs[child].cached {
                    if workers[worker].parallelism > 0 {
                        let bytes = present.entry(worker).or_default();
                        *bytes = bytes.saturating_add(output);
                    }
                }
            }
        }
        let local = present.get(&0).copied().unwrap_or(0);
        let choice = present
            .iter()
            .filter(|(worker, _)| eligible(job, &workers[**worker]))
            .max_by_key(|(worker, bytes)| {
                (
                    **bytes,
                    workers[**worker].parallelism,
                    **worker == 0,
                    core::cmp::Reverse(**worker),
                )
            })
            .map_or_else(
                || {
                    (
                        workers
                            .iter()
                            .position(|worker| eligible(job, worker))
                            .unwrap_or(0),
                        0,
                    )
                },
                |(&worker, &bytes)| {
                    (
                        worker,
                        if worker == 0 {
                            local
                        } else {
                            bytes.saturating_sub(local)
                        },
                    )
                },
            );
        assignments.insert(request, choice);
    }
}

impl<'a> OnePass<'a> {
    /// Use the supplied graph for local dependency evaluation.
    pub fn new(graph: &'a Graph) -> Result<Self, Error> {
        Ok(Self {
            graph,
            parallel: Parallel::new(graph)?,
            advice: RefCell::default(),
            sketch: RefCell::new(None),
        })
    }

    /// Retain coupons using the local scheduler's advice and memoization choices.
    pub fn retain(&self, coupon: &fixwasm::backend::Tree) -> Result<(), Error> {
        self.parallel.retain(coupon)
    }

    async fn cluster(&self) -> Result<alloc::rc::Rc<Cluster>, Error> {
        self.parallel.cluster().await
    }

    /// Connect to configured remotes and report local and remote worker information.
    pub async fn information(&self) -> Result<Vec<Information>, Error> {
        let cluster = self.cluster().await?;
        let mut workers = vec![Information {
            parallelism: fixscheduler::kernel::worker::count()?.try_into()?,
            ..Information::default()
        }];
        workers.extend(cluster.information());
        Ok(workers)
    }

    /// Dispatch assigned subgraphs in locality order, stopping traversal at remote roots.
    /// Local work and failed delegations use Parallel's evaluator.
    pub async fn dispatch(
        &self,
        request: &Node,
        jobs: &BTreeMap<Node, Job>,
        assignments: &BTreeMap<Node, (usize, u64)>,
    ) -> Result<(), Error> {
        let mut pending = vec![request.clone()];
        let mut seen = BTreeSet::new();
        let mut remote = Vec::new();
        let mut local = Vec::new();
        while let Some(request) = pending.pop() {
            if !seen.insert(request.clone()) {
                continue;
            }
            match assignments.get(&request) {
                Some(&(worker, score)) if worker != 0 => remote.push((score, worker, request)),
                _ => {
                    if let Some(&(0, score)) = assignments.get(&request) {
                        local.push((score, request.clone()));
                    }
                    pending.extend(jobs[&request].dependencies.iter().cloned());
                }
            }
        }
        remote.sort_by_key(|(score, _, _)| core::cmp::Reverse(*score));
        for (_, worker, request) in remote {
            let cluster = self.cluster().await?;
            let outcome = cluster
                .execute(worker, request.code(), request.input())
                .await;
            if let Ok(Some(coupon)) = outcome {
                self.parallel
                    .retain(&fixwasm::backend::Tree::try_from(coupon.clone())?)?;
                self.graph.complete(&request, &coupon)?;
            }
        }
        local.sort_by_key(|(score, _)| core::cmp::Reverse(*score));
        for (_, child) in local {
            if &child != request && matches!(self.graph.claim(&child)?, Claim::Owned) {
                self.graph.offer(child)?;
            }
        }
        Ok(())
    }

    /// Ask the evaluator for available dependencies without fetching data or executing application leaves.
    pub fn discover<F: Future<Output = Result<Value, Error>>>(
        &self,
        request: &Node,
        workers: &[Information],
        evaluate: impl Fn() -> F,
    ) -> Result<BTreeMap<Node, Job>, Error> {
        *self.sketch.borrow_mut() = Some(BTreeMap::new());
        let result = {
            let mut future = alloc::boxed::Box::pin(self.get(request, &evaluate));
            future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
        };
        let mut jobs = self.sketch.borrow_mut().take().unwrap();
        if let Poll::Ready(result) = result {
            result?;
        }
        let collector = storage::canonical_value(&collector::callable()?)?.raw_handle_bytes()?;
        for (request, job) in &mut jobs {
            let coupon = match self.graph.completed(request)? {
                Some(coupon) => Some(coupon),
                None => collector::claims(request.code(), request.input())?
                    .into_iter()
                    .next(),
            };
            job.output_bytes = 1;
            let input = storage::canonical_value(request.input())?.raw_handle_bytes()?;
            for (index, worker) in workers.iter().enumerate().skip(1) {
                if worker.results.iter().any(|task| {
                    task.operation == request.code()
                        && task.input == input
                        && task.collector == collector
                }) {
                    job.cached.insert(index);
                }
            }
            if let Some(coupon) = coupon {
                job.cached.insert(0);
                job.output_bytes = storage::identify(&fixscheduler::collector::result(&coupon)?)?
                    .map(|value| storage::byte_size(&value))
                    .transpose()?
                    .unwrap_or(1);
            } else {
                let operand = if request.is_select() {
                    match storage::identify(request.input())? {
                        Some(arguments) => Some(child(&arguments, 0)?),
                        None => None,
                    }
                } else {
                    None
                };
                let data = operand.as_ref().unwrap_or(request.input());
                let target = storage::identify(data)?;
                let mut root = storage::canonical_value(data)?.raw_handle_bytes()?;
                root[31] &= 0xe0;
                let present = workers
                    .iter()
                    .enumerate()
                    .filter_map(|(index, worker)| {
                        (if index == 0 {
                            target.is_some()
                        } else {
                            worker.data.contains(&root)
                        })
                        .then_some(index)
                    })
                    .collect::<Vec<_>>();
                if !present.is_empty() {
                    let size = storage::byte_size(target.as_ref().unwrap_or(data))?;
                    for worker in present {
                        job.present.insert(worker, size);
                    }
                } else {
                    let inventory = storage::inventory(data)?;
                    for (index, worker) in workers.iter().enumerate() {
                        let bytes = inventory
                            .required
                            .iter()
                            .filter(|(name, _)| {
                                if index == 0 {
                                    inventory.local.contains(name)
                                } else {
                                    worker.data.contains(name)
                                }
                            })
                            .fold(0u64, |sum, (_, bytes)| sum.saturating_add(*bytes));
                        if bytes > 0 {
                            job.present.insert(index, bytes);
                        }
                    }
                }
                if let Some(target) = storage::identify(request.input())? {
                    let read = |name: &[u8]| -> Result<Option<u64>, Error> {
                        let key = storage::canonical_value(&fixscheduler::shell::capture(tree![
                            &target,
                            fixwasm::Blob::create(name)
                        ])?)?;
                        Ok(self.advice.borrow().get(&key.raw_handle_bytes()?).copied())
                    };
                    job.output_bytes = read(b"fix::output-size")?.unwrap_or(1);
                    job.parallel = read(b"fix::parallel")?.is_some_and(|n| n != 0);
                    if matches!(
                        request.input().value_type(),
                        ValueType::Thunk | ValueType::Encode
                    ) && request.input().unsound_thunk_type()? == ThunkType::Application
                        && target.value_type() == ValueType::Object
                        && target.data_type()? == DataType::Tree
                        && target.len()? > 0
                    {
                        job.memory_bytes = unsigned(&child(&target, 0)?)?;
                    }
                }
            }
        }
        Ok(jobs)
    }

    /// Assign the available dependency graph by locality, then delegate selected subgraphs.
    pub async fn schedule<F: Future<Output = Result<Value, Error>>>(
        &self,
        request: &Node,
        evaluate: impl Fn() -> F,
    ) -> Result<Value, Error> {
        let mut execution = alloc::boxed::Box::pin(self.get(request, &evaluate));
        loop {
            if let Some(coupon) = self.graph.completed(request)? {
                return Ok(coupon);
            }
            let workers = self.information().await?;
            if workers.iter().skip(1).all(|worker| worker.parallelism == 0) {
                return execution.await;
            }
            let jobs = self.discover(request, &workers, &evaluate)?;
            let mut assignments = BTreeMap::new();
            assign(request, &jobs, &workers, &mut assignments);
            self.dispatch(request, &jobs, &assignments).await?;
            if let Some(coupon) = self.graph.completed(request)? {
                return Ok(coupon);
            }
            let mut polled = false;
            let result = core::future::poll_fn(|cx| {
                if polled {
                    return Poll::Ready(None);
                }
                polled = true;
                match execution.as_mut().poll(cx) {
                    Poll::Ready(result) => Poll::Ready(Some(result)),
                    Poll::Pending => Poll::Pending,
                }
            })
            .await;
            if let Some(result) = result {
                return result;
            }
        }
    }
    /// Discover dependencies without fetching or running leaves; use Parallel when executing.
    pub async fn get<F: Future<Output = Result<Value, Error>>>(
        &self,
        request: &Node,
        evaluate: impl Fn() -> F,
    ) -> Result<Value, Error> {
        if self.sketch.borrow().is_none() {
            if let Some(coupon) = self.graph.completed(request)? {
                return Ok(coupon);
            }
            if let Some(coupon) = collector::claims(request.code(), request.input())?
                .into_iter()
                .next()
            {
                fixscheduler::kernel::cached(request.code(), request.input())?;
                self.graph.complete(request, &coupon)?;
                return Ok(coupon);
            }
            if self.graph.current().is_some_and(|parent| {
                request.is_apply() || request.is_select() || (request.is_eval() && parent.is_eval())
            }) {
                return self.graph.defer(request).await;
            }
            return self.graph.evaluate(request, evaluate()).await;
        }
        let inserted = {
            let mut sketch = self.sketch.borrow_mut();
            let jobs = sketch.as_mut().unwrap();
            if let Some(parent) = self.graph.current().as_ref() {
                jobs.get_mut(parent)
                    .unwrap()
                    .dependencies
                    .insert(request.clone());
            }
            if jobs.contains_key(request) {
                false
            } else {
                jobs.insert(request.clone(), Job::default());
                true
            }
        };
        let mut result = match self.graph.completed(request)? {
            Some(coupon) => Some(coupon),
            None => collector::claims(request.code(), request.input())?
                .into_iter()
                .next(),
        };
        if result.is_none() && inserted && !(request.is_apply() || request.is_select()) {
            result = Some(self.graph.within(request, evaluate()).await?);
        }
        if let Some(coupon) = &result {
            self.graph.complete(request, coupon)?;
            let mut sketch = self.sketch.borrow_mut();
            let jobs = sketch.as_mut().unwrap();
            jobs.get_mut(request).unwrap().dependencies.clear();
            if request.is_eval()
                && request.input().is_value()?
                && let Some(parent) = self.graph.current().as_ref()
            {
                jobs.get_mut(parent).unwrap().dependencies.remove(request);
            }
        }
        match result {
            Some(value) => Ok(value),
            None => core::future::pending().await,
        }
    }
}

/// Run this scheduler and retain the final coupon when requested by the caller.
pub fn run(combination: &fixwasm::Handle<fixwasm::Object<fixwasm::Tree>>) -> Result<Value, Error> {
    use fixscheduler::{executor, kernel::worker, storage};
    let graph = Graph::default();
    let io = fixscheduler::io::Io;
    let policy = OnePass::new(&graph)?;
    let result = executor::run(combination, &policy, &graph, &io)?;
    let raw = fixwasm::Value::equals(
        &fixwasm::Value::get(combination, 2),
        fixwasm::Blob::create(b"load"),
    )?;
    let result = if !raw
        && !worker::returns_coupon()?
        && !fixwasm::Value::equals(
            &fixwasm::Value::get(combination, 2),
            fixwasm::Blob::create(b"todo"),
        )? {
        fixscheduler::collector::result(&result)?
    } else {
        result
    };
    if !raw && worker::returns_coupon()? {
        storage::cache(&result, true)?;
    }
    worker::finish(&result)?;
    Ok(result)
}
