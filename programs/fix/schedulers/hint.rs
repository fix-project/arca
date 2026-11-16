use crate::onepass::{self, Job, eligible};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec,
    vec::Vec,
};
use core::{future::Future, task::Poll};
use fixscheduler::collector;
use fixscheduler::executor::Graph;
use fixscheduler::executor::graph::Node;
use fixscheduler::{
    executor::Scheduler,
    kernel::Advice,
    remote::protocol::Information,
    shell::{AsValue, DataType, Value, ValueType},
};
use fixwasm::Error;

pub struct Hint<'a> {
    graph: &'a Graph,
    onepass: onepass::OnePass<'a>,
}

impl Scheduler for Hint<'_> {
    /// Resolve eval using this scheduler's evaluation and placement choices.
    async fn eval(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::eval(input)?;
        if self.graph.current().is_none() {
            self.schedule(&node, || async {
                crate::parallel::Parallel::eval_using(&self.onepass, input, &mut |coupon| {
                    self.onepass.retain(coupon)
                })
                .await
            })
            .await
        } else {
            self.onepass
                .get(&node, || async {
                    crate::parallel::Parallel::eval_using(&self.onepass, input, &mut |coupon| {
                        self.onepass.retain(coupon)
                    })
                    .await
                })
                .await
        }
    }
    /// Resolve think using this scheduler's evaluation and placement choices.
    async fn think(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::think(input)?;
        if self.graph.current().is_none() {
            self.schedule(&node, || async {
                collector::think(&self.onepass, input, &mut |coupon| {
                    self.onepass.retain(coupon)
                })
                .await
            })
            .await
        } else {
            self.onepass
                .get(&node, || async {
                    collector::think(&self.onepass, input, &mut |coupon| {
                        self.onepass.retain(coupon)
                    })
                    .await
                })
                .await
        }
    }
    /// Resolve force using this scheduler's evaluation and placement choices.
    async fn force(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::force(input)?;
        if self.graph.current().is_none() {
            self.schedule(&node, || async {
                collector::force(&self.onepass, input, &mut |coupon| {
                    self.onepass.retain(coupon)
                })
                .await
            })
            .await
        } else {
            self.onepass
                .get(&node, || async {
                    collector::force(&self.onepass, input, &mut |coupon| {
                        self.onepass.retain(coupon)
                    })
                    .await
                })
                .await
        }
    }
    /// Resolve apply using this scheduler's evaluation and placement choices.
    async fn apply(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::apply(input)?;
        if self.graph.current().is_none() {
            self.schedule(&node, || async {
                collector::apply(&self.onepass, input, &mut |coupon| {
                    self.onepass.retain(coupon)
                })
                .await
            })
            .await
        } else {
            self.onepass
                .get(&node, || async {
                    collector::apply(&self.onepass, input, &mut |coupon| {
                        self.onepass.retain(coupon)
                    })
                    .await
                })
                .await
        }
    }
    /// Resolve select using this scheduler's evaluation and placement choices.
    async fn select(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::select(input)?;
        if self.graph.current().is_none() {
            self.schedule(&node, || async {
                collector::select(input, &mut |coupon| self.onepass.retain(coupon)).await
            })
            .await
        } else {
            self.onepass
                .get(&node, || async {
                    collector::select(input, &mut |coupon| self.onepass.retain(coupon)).await
                })
                .await
        }
    }

    /// Retrieve data using the composed scheduler's source order.
    async fn load(&self, value: &Value) -> Result<Value, Error> {
        self.onepass.load(value).await
    }
    /// Forward advisory notifications to the composed scheduler.
    fn on_advice(&self, advice: &Advice) -> Result<(), Error> {
        self.onepass.on_advice(advice)
    }
}

/// Start with OnePass, reconsider parents around their children, then distribute excess local work.
pub fn assign(
    root: &Node,
    jobs: &BTreeMap<Node, Job>,
    workers: &[Information],
    assignments: &mut BTreeMap<Node, (usize, u64)>,
) {
    onepass::assign(root, jobs, workers, assignments);
    pull_back_outputs(root, jobs, workers, assignments);
    spread_independent(root, jobs, workers, assignments);
}

/// Pull back expensive outputs; after visiting children, reconsider a parent's destination and score.
pub fn pull_back_outputs(
    root: &Node,
    jobs: &BTreeMap<Node, Job>,
    workers: &[Information],
    assignments: &mut BTreeMap<Node, (usize, u64)>,
) {
    let mut pending = vec![(root.clone(), false)];
    let mut seen = BTreeSet::new();
    while let Some((request, visited)) = pending.pop() {
        let job = &jobs[&request];
        let Some(&(worker, score)) = assignments.get(&request) else {
            if seen.insert(request) {
                pending.extend(job.dependencies.iter().map(|child| (child.clone(), false)));
            }
            continue;
        };
        if !visited {
            if !seen.insert(request.clone()) {
                continue;
            }
            if worker != 0 && !job.parallel && eligible(job, &workers[0]) {
                if job.cached.contains(&0) {
                    assignments.insert(request.clone(), (0, job.output_bytes));
                } else if job.cached.is_empty() && score <= job.output_bytes {
                    assignments.insert(request.clone(), (0, job.output_bytes - score));
                }
            }
            if assignments[&request].0 == 0 {
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
        if job.parallel {
            continue;
        }
        let tree = request.is_eval()
            && request.input().value_type() == ValueType::Object
            && request.input().data_type().ok() == Some(DataType::Tree);
        let undo = score
            == if tree {
                64 * job.dependencies.len() as u64
            } else {
                0
            };
        if undo {
            let choice = if job.dependencies.len() == 1 {
                let child = job.dependencies.first().unwrap();
                assignments.get(child).copied().or_else(|| {
                    if jobs[child].cached.contains(&0) {
                        None
                    } else {
                        jobs[child]
                            .cached
                            .iter()
                            .find(|worker| workers[**worker].parallelism > 0)
                            .map(|worker| (*worker, 0))
                    }
                })
            } else {
                let mut choices = job
                    .dependencies
                    .iter()
                    .filter_map(|child| assignments.get(child));
                choices.next().and_then(|&(worker, _)| {
                    choices
                        .all(|(other, _)| *other == worker)
                        .then_some((worker, 0))
                })
            };
            if let Some((worker, score)) = choice
                && eligible(job, &workers[worker])
            {
                assignments.insert(request, (worker, score));
            }
        } else {
            let score = job
                .dependencies
                .iter()
                .filter_map(|child| assignments.get(child))
                .filter(|(worker, _)| *worker == 0)
                .fold(score, |sum, (_, score)| sum.saturating_add(*score));
            assignments.insert(request, (0, score));
        }
    }
}

/// Retain twice the local parallelism and send lower-locality siblings round-robin to a sufficient remote prefix.
pub fn spread_independent(
    root: &Node,
    jobs: &BTreeMap<Node, Job>,
    workers: &[Information],
    assignments: &mut BTreeMap<Node, (usize, u64)>,
) {
    let remotes: Vec<_> = (1..workers.len())
        .filter(|worker| workers[*worker].parallelism > 0)
        .collect();
    if remotes.is_empty() {
        return;
    }
    let local_jobs = workers[0].parallelism as usize * 2;
    let mut pending = vec![root.clone()];
    let mut seen = BTreeSet::new();
    while let Some(request) = pending.pop() {
        if !seen.insert(request.clone())
            || assignments
                .get(&request)
                .is_some_and(|(worker, _)| *worker != 0)
        {
            continue;
        }
        let dependencies = &jobs[&request].dependencies;
        let mut parallel = false;
        for child in dependencies {
            if jobs[child].parallel {
                assignments.insert(child.clone(), (0, 0));
                parallel = true;
            }
        }
        if !parallel && dependencies.len() >= local_jobs {
            let mut local: Vec<_> = dependencies
                .iter()
                .filter_map(|child| {
                    assignments
                        .get(child)
                        .filter(|(worker, _)| *worker == 0)
                        .map(|(_, score)| (*score, child))
                })
                .collect();
            local.sort_by_key(|(score, _)| *score);
            let mut assigned = local.len();
            let mut capacity = workers[0].parallelism as usize;
            let mut count = 0;
            while count < remotes.len() {
                capacity += workers[remotes[count]].parallelism as usize;
                count += 1;
                if capacity > assigned {
                    break;
                }
            }
            let mut next = 0;
            for (score, child) in local {
                if assigned <= local_jobs {
                    break;
                }
                if let Some(offset) = (0..count).find(|offset| {
                    eligible(&jobs[child], &workers[remotes[(next + offset) % count]])
                }) {
                    let index = (next + offset) % count;
                    assignments.insert(child.clone(), (remotes[index], score));
                    assigned -= 1;
                    next = (index + 1) % count;
                }
            }
        }
        pending.extend(dependencies.iter().rev().cloned());
    }
}

impl<'a> Hint<'a> {
    /// Refine OnePass's assignments by propagating output locality and distributing wide work.
    async fn schedule<F: Future<Output = Result<Value, Error>>>(
        &self,
        request: &Node,
        evaluate: impl Fn() -> F,
    ) -> Result<Value, Error> {
        let mut execution = alloc::boxed::Box::pin(self.onepass.get(request, &evaluate));
        loop {
            if let Some(coupon) = self.graph.completed(request)? {
                return Ok(coupon);
            }
            let workers = self.onepass.information().await?;
            if workers.iter().skip(1).all(|worker| worker.parallelism == 0) {
                return execution.await;
            }
            let jobs = self.onepass.discover(request, &workers, &evaluate)?;
            let mut assignments = BTreeMap::new();
            assign(request, &jobs, &workers, &mut assignments);
            self.onepass.dispatch(request, &jobs, &assignments).await?;
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
}

/// Run this scheduler and retain the final coupon when requested by the caller.
pub fn run(combination: &fixwasm::Handle<fixwasm::Object<fixwasm::Tree>>) -> Result<Value, Error> {
    use fixscheduler::{executor, kernel::worker, storage};
    let graph = Graph::default();
    let io = fixscheduler::io::Io;
    let policy = Hint {
        graph: &graph,
        onepass: onepass::OnePass::new(&graph, &io)?,
    };
    let result = executor::run(combination, &policy, &graph, &io)?;
    let result = if !worker::returns_coupon()?
        && !fixwasm::Value::equals(
            &fixwasm::Value::get(combination, 2),
            fixwasm::Blob::create(b"todo"),
        )? {
        fixscheduler::collector::result(&result)?
    } else {
        result
    };
    if worker::returns_coupon()? {
        storage::cache(&result, true)?;
    }
    worker::finish(&result)?;
    Ok(result)
}
