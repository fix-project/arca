use crate::parallel::Parallel;
use alloc::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    vec,
    vec::Vec,
};
use core::cell::{Cell, RefCell};
use core::{
    future::Future,
    pin::Pin,
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
    remote::{
        Cluster, Failure,
        protocol::{self, Information},
    },
    shell::{AsValue, DataType, Value, ValueType, child},
    storage,
};
use fixwasm::{Error, ThunkType, tree};

pub struct OnePass<'a> {
    graph: &'a Graph,
    parallel: Parallel<'a>,
    advice: RefCell<BTreeMap<[u8; 32], u64>>,
    sketch: RefCell<Option<BTreeMap<Node, Job>>>,
    placed_locally: RefCell<BTreeSet<Node>>,
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
    async fn apply_coupon(&self, input: &Value) -> Result<Value, Error> {
        let mut attempted = BTreeSet::new();
        fixscheduler::kernel::apply_with_fallback(
            input,
            &mut |advice| self.on_advice(advice),
            async |required| {
                let cluster = self.cluster().await?;
                for worker in cluster.workers_with_memory(required) {
                    if attempted.insert(worker)
                        && let Ok(Some(coupon)) = cluster.execute(worker, 2, input, None).await
                    {
                        return Ok(Some(coupon));
                    }
                }
                Ok(None)
            },
        )
        .await
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

    fn discovering(&self) -> bool {
        self.sketch.borrow().is_some()
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
    pub parallel: u64,
    require_resident_operand: bool,
    in_flight: bool,
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

fn eligible(job: &Job, index: usize, worker: &Information) -> bool {
    worker.parallelism > 0
        && (!job.require_resident_operand || job.present.contains_key(&index))
        && match (job.memory_bytes, worker.memory_capacity_bytes) {
            (Some(required), Some(available)) => required <= available,
            _ => true,
        }
}

fn available(job: &Job, index: usize, worker: &Information) -> bool {
    eligible(job, index, worker)
        && match (job.memory_bytes, worker.memory_bytes) {
            (Some(required), Some(available)) => required <= available,
            _ => true,
        }
}

pub fn candidate(job: &Job, index: usize, workers: &[Information]) -> bool {
    let worker = &workers[index];
    eligible(job, index, worker)
        && (available(job, index, worker)
            || (!workers
                .iter()
                .enumerate()
                .any(|(index, worker)| available(job, index, worker))
                && !workers.iter().enumerate().any(|(index, other)| {
                    eligible(job, index, other)
                        && other.memory_capacity_bytes > worker.memory_capacity_bytes
                })))
}

/// Assign dependencies before parents, preferring cached results and then resident bytes.
pub fn assign(
    root: &Node,
    jobs: &BTreeMap<Node, Job>,
    workers: &[Information],
    assignments: &mut BTreeMap<Node, (usize, u64)>,
) -> Result<(), Node> {
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
        if job.in_flight {
            continue;
        }
        if !(request.is_eval()
            || request.is_apply()
            || request.is_think()
            || request.is_select()
            || request.code() == 3)
        {
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
        if (request.code() == 3
            || (request.is_eval() && request.input().value_type() == ValueType::Encode))
            && job.dependencies.len() == 1
            && let Some(choice) = assignments.get(job.dependencies.first().unwrap()).copied()
            && candidate(job, choice.0, workers)
        {
            assignments.insert(request, choice);
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
        let parallelism = |worker: usize| {
            let capacity = u64::from(workers[worker].parallelism);
            if job.parallel == 0 {
                capacity
            } else {
                capacity.min(job.parallel)
            }
        };
        let choice = present
            .iter()
            .filter(|(worker, _)| candidate(job, **worker, workers))
            .max_by_key(|(worker, bytes)| {
                (
                    **bytes,
                    parallelism(**worker),
                    **worker == 0,
                    core::cmp::Reverse(**worker),
                )
            })
            .map_or_else(
                || {
                    let worker = if job.parallel == 0 {
                        (0..workers.len()).find(|&worker| candidate(job, worker, workers))
                    } else {
                        (0..workers.len())
                            .filter(|&worker| candidate(job, worker, workers))
                            .max_by_key(|worker| {
                                (
                                    parallelism(*worker),
                                    *worker == 0,
                                    core::cmp::Reverse(*worker),
                                )
                            })
                    };
                    worker.map(|worker| (worker, 0))
                },
                |(&worker, &bytes)| {
                    Some((
                        worker,
                        if worker == 0 {
                            local
                        } else {
                            bytes.saturating_sub(local)
                        },
                    ))
                },
            );
        let Some(choice) = choice else {
            return Err(request);
        };
        assignments.insert(request, choice);
    }
    Ok(())
}

fn output_size(
    request: &Node,
    jobs: &BTreeMap<Node, Job>,
    sizes: &mut BTreeMap<Node, u64>,
) -> Result<u64, Error> {
    if let Some(size) = sizes.get(request) {
        return Ok(*size);
    }
    let job = &jobs[request];
    sizes.insert(request.clone(), job.output_bytes);
    let mut children = 0u64;
    let mut largest = 1u64;
    for child in &job.dependencies {
        let size = output_size(child, jobs, sizes)?;
        children = children.saturating_add(size);
        largest = largest.max(size);
    }
    let input = request.input();
    let size = if job.cached.contains(&0) || job.output_bytes != 1 {
        job.output_bytes
    } else if matches!(input.value_type(), ValueType::Thunk | ValueType::Encode)
        && input.unsound_thunk_type()? == ThunkType::Identification
    {
        storage::byte_size(input)?
    } else if request.is_eval() {
        if input.value_type() == ValueType::Encode {
            largest
        } else {
            storage::byte_size(input)?.saturating_add(children)
        }
    } else if request.code() == 3 {
        largest
    } else {
        job.output_bytes
    };
    sizes.insert(request.clone(), size);
    Ok(size)
}

impl<'a> OnePass<'a> {
    /// Use the supplied graph for local dependency evaluation.
    pub fn new(graph: &'a Graph) -> Result<Self, Error> {
        Ok(Self {
            graph,
            parallel: Parallel::new(graph)?,
            advice: RefCell::default(),
            sketch: RefCell::new(None),
            placed_locally: RefCell::default(),
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
        Self::worker_information(&cluster)
    }

    fn worker_information(cluster: &Cluster) -> Result<Vec<Information>, Error> {
        let mut workers = vec![Information {
            parallelism: fixscheduler::kernel::worker::count()?.try_into()?,
            memory_bytes: Some(fixscheduler::kernel::worker::available_memory()?),
            memory_capacity_bytes: Some(fixscheduler::kernel::worker::memory_capacity()?),
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
        workers: &[Information],
        assignments: &BTreeMap<Node, (usize, u64)>,
    ) -> Result<(), Error> {
        let root = request;
        let mut pending = vec![root.clone()];
        let mut seen = BTreeSet::new();
        let mut remote = Vec::new();
        let mut local = Vec::new();
        while let Some(request) = pending.pop() {
            if !seen.insert(request.clone()) {
                continue;
            }
            match assignments.get(&request) {
                Some(&(worker, score)) if worker != 0 => {
                    remote.push((score, worker, request));
                }
                _ => {
                    if let Some(&(0, score)) = assignments.get(&request) {
                        local.push((score, request.clone()));
                    }
                    pending.extend(jobs[&request].dependencies.iter().cloned());
                }
            }
        }
        local.sort_by_key(|(score, _)| core::cmp::Reverse(*score));
        for (_, child) in local {
            if &child != root && matches!(self.graph.claim(&child)?, Claim::Owned) {
                self.placed_locally.borrow_mut().insert(child.clone());
                self.graph.offer(child)?;
            }
        }
        remote.sort_by_key(|(score, ..)| core::cmp::Reverse(*score));
        let mut by_worker: BTreeMap<_, Vec<Node>> = BTreeMap::new();
        for (_, worker, request) in remote {
            by_worker.entry(worker).or_default().push(request);
        }
        type Transfer<'a> = Pin<Box<dyn Future<Output = Result<(), Error>> + 'a>>;
        let mut transfers: Vec<Transfer<'_>> = Vec::new();
        for (worker, requests) in by_worker {
            let slots = requests.len().min(
                (workers[worker].parallelism as usize)
                    .saturating_mul(2)
                    .max(1),
            );
            let pending = Rc::new(RefCell::new(requests.into_iter()));
            let active = Rc::new(Cell::new(slots));
            let inventory = fixscheduler::shell::capture(fixwasm::Blob::create(
                workers[worker].data.as_flattened(),
            ))?;
            let cluster = self.cluster().await?;
            for _ in 0..slots {
                let pending = pending.clone();
                let active = active.clone();
                let inventory = inventory.clone();
                let cluster = cluster.clone();
                transfers.push(Box::pin(async move {
                    loop {
                        let request = pending.borrow_mut().next();
                        let Some(request) = request else {
                            active.set(active.get() - 1);
                            if active.get() == 0 {
                                cluster.report_final_progress(worker).await;
                            }
                            return Ok(());
                        };
                        let reservation = if &request == root {
                            None
                        } else {
                            let Some(owner) = self.graph.reserve(&request)? else {
                                continue;
                            };
                            Some(owner)
                        };
                        let result = cluster
                            .execute(worker, request.code(), request.input(), Some(&inventory))
                            .await;
                        let coupon = match result {
                            Ok(Some(coupon)) => Some(coupon),
                            failed => {
                                self.retry_delegation(
                                    &cluster,
                                    worker,
                                    &request,
                                    &jobs[&request],
                                    failed,
                                )
                                .await?
                            }
                        };
                        if let Some(coupon) = coupon {
                            self.parallel
                                .retain(&fixwasm::backend::Tree::try_from(coupon.clone())?)?;
                            self.graph.complete(&request, &coupon)?;
                        } else if reservation.is_some() {
                            self.placed_locally.borrow_mut().insert(request);
                        }
                        drop(reservation);
                    }
                }));
            }
        }
        fixscheduler::executor::task::try_join_all(transfers).await?;
        Ok(())
    }

    async fn retry_delegation(
        &self,
        cluster: &Cluster,
        mut worker: usize,
        request: &Node,
        job: &Job,
        mut result: Result<Option<Value>, Failure>,
    ) -> Result<Option<Value>, Error> {
        let mut failures = Vec::new();
        let mut attempted = BTreeSet::new();
        loop {
            let reason = match result {
                Ok(Some(coupon)) => return Ok(Some(coupon)),
                Ok(None) => alloc::string::String::from("worker disconnected"),
                Err(
                    Failure::Send(protocol::Error::Remote(message))
                    | Failure::Receive(protocol::Error::Remote(message)),
                ) => message,
                Err(Failure::Send(error) | Failure::Receive(error)) => alloc::format!("{error}"),
                Err(Failure::Export(message) | Failure::Import(message)) => message,
                Err(Failure::AwaitingResult) => "worker is already awaiting this evaluation".into(),
                Err(error) => alloc::format!("{error:?}"),
            };
            let location = cluster
                .location(worker)
                .unwrap_or_else(|| alloc::format!("worker {worker}"));
            failures.push(alloc::format!("{location}: {reason}"));
            attempted.insert(worker);
            let workers = Self::worker_information(cluster)?;
            if available(job, 0, &workers[0]) {
                return Ok(None);
            }
            let alternatives = || {
                workers.iter().enumerate().skip(1).filter(|(index, other)| {
                    !attempted.contains(index) && eligible(job, *index, other)
                })
            };
            if let Some((next, _)) =
                alternatives().find(|(index, other)| available(job, *index, other))
            {
                worker = next;
            } else if eligible(job, 0, &workers[0]) {
                return Ok(None);
            } else if let Some((next, _)) =
                alternatives().max_by_key(|(_, other)| other.memory_capacity_bytes)
            {
                worker = next;
            } else {
                let application = request
                    .application_description()
                    .ok()
                    .map(|application| alloc::format!(" for {application}"))
                    .unwrap_or_default();
                let reasons = failures.join("; ");
                return Err(self.graph.fail(
                    Error::WrongType,
                    alloc::format!("delegation failed{application}: {reasons}"),
                ));
            }
            result = cluster
                .execute(worker, request.code(), request.input(), None)
                .await;
        }
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
            if job.in_flight {
                continue;
            }
            let coupon = match self.graph.completed(request)? {
                Some(coupon) => Some(coupon),
                None => collector::claims(request.code(), request.input())?
                    .into_iter()
                    .next(),
            };
            job.output_bytes = 1;
            let input = storage::canonical_value(request.input())?.raw_handle_bytes()?;
            for (index, worker) in workers.iter().enumerate().skip(1) {
                if worker
                    .results
                    .binary_search_by_key(&(request.code(), input, collector), |task| {
                        (task.operation, task.input, task.collector)
                    })
                    .is_ok()
                {
                    job.cached.insert(index);
                }
            }
            if let Some(coupon) = coupon {
                job.cached.insert(0);
                job.output_bytes = storage::byte_size(&fixscheduler::collector::result(&coupon)?)?;
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
                            worker.data.binary_search(&root).is_ok()
                        })
                        .then_some(index)
                    })
                    .collect::<Vec<_>>();
                if !present.is_empty() {
                    job.require_resident_operand = operand
                        .as_ref()
                        .is_some_and(|operand| operand.value_type() == ValueType::Ref)
                        && present
                            .iter()
                            .any(|&worker| workers[worker].parallelism > 0);
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
                                    worker.data.binary_search(name).is_ok()
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
                    job.parallel = read(b"fix::parallel")?.unwrap_or(0);
                    if request.is_application()?
                        && target.value_type() == ValueType::Object
                        && target.data_type()? == DataType::Tree
                        && target.len()? > 0
                    {
                        job.memory_bytes = unsigned(&child(&target, 0)?)?;
                    }
                }
            }
        }
        let mut sizes = BTreeMap::new();
        output_size(request, &jobs, &mut sizes)?;
        for (request, size) in sizes {
            jobs.get_mut(&request).unwrap().output_bytes = size;
        }
        Ok(jobs)
    }

    /// Assign the available dependency graph by locality, then delegate selected subgraphs.
    pub async fn schedule<F: Future<Output = Result<Value, Error>>>(
        &self,
        request: &Node,
        evaluate: impl Fn() -> F,
    ) -> Result<Value, Error> {
        self.schedule_with(request, evaluate, assign).await
    }

    pub async fn schedule_with<F: Future<Output = Result<Value, Error>>>(
        &self,
        request: &Node,
        evaluate: impl Fn() -> F,
        assign: impl FnOnce(
            &Node,
            &BTreeMap<Node, Job>,
            &[Information],
            &mut BTreeMap<Node, (usize, u64)>,
        ) -> Result<(), Node>,
    ) -> Result<Value, Error> {
        let placed = self.placed_locally.borrow_mut().remove(request);
        if let Some(coupon) = self.graph.completed(request)? {
            return Ok(coupon);
        }
        let execution = self.get(request, &evaluate);
        if placed {
            return execution.await;
        }
        let workers = self.information().await?;
        if workers.iter().skip(1).all(|worker| worker.parallelism == 0) {
            return execution.await;
        }
        let jobs = self.discover(request, &workers, &evaluate)?;
        let mut assignments = BTreeMap::new();
        if let Err(failed) = assign(request, &jobs, &workers, &mut assignments) {
            let application = failed
                .application_description()
                .ok()
                .map(|application| alloc::format!(" for {application}"))
                .unwrap_or_default();
            let capacity = workers
                .iter()
                .filter(|worker| worker.parallelism > 0)
                .filter_map(|worker| worker.memory_capacity_bytes)
                .max()
                .unwrap_or(0);
            return Err(self.graph.fail(Error::MemoryUnavailable, alloc::format!(
                "memory admission rejected{application}: no eligible worker (largest advertised capacity {capacity} bytes)"
            )));
        }
        let dispatch = self.dispatch(request, &jobs, &workers, &assignments);
        if assignments
            .get(request)
            .is_some_and(|(worker, _)| *worker != 0)
        {
            dispatch.await?;
            return execution.await;
        }
        let mut dispatch = core::pin::pin!(dispatch);
        let mut execution = core::pin::pin!(execution);
        let mut dispatched = false;
        let mut result = None;
        core::future::poll_fn(|cx| {
            if !dispatched && let Poll::Ready(sent) = dispatch.as_mut().poll(cx) {
                sent?;
                dispatched = true;
            }
            if result.is_none()
                && let Poll::Ready(value) = execution.as_mut().poll(cx)
            {
                result = Some(value?);
            }
            if dispatched && let Some(value) = result.take() {
                return Poll::Ready(Ok(value));
            }
            Poll::Pending
        })
        .await
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
        let in_flight = self.graph.scheduled_elsewhere(request);
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
                jobs.insert(
                    request.clone(),
                    Job {
                        in_flight,
                        ..Job::default()
                    },
                );
                true
            }
        };
        let mut result = match self.graph.completed(request)? {
            Some(coupon) => Some(coupon),
            None => collector::claims(request.code(), request.input())?
                .into_iter()
                .next(),
        };
        if result.is_none()
            && inserted
            && !in_flight
            && !(request.is_apply() || request.is_select())
        {
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
