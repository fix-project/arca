#![no_std]
extern crate alloc;
use fixscheduler::collector;
use fixscheduler::executor::graph::Node;
use fixscheduler::shell::Value;
#[path = "../parallel.rs"]
pub mod parallel;
use fixscheduler::executor::{self, Scheduler};
use fixwasm::{Error, Handle, Object};
#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

struct Recompute<'a> {
    graph: &'a fixscheduler::executor::Graph,
    parallel: parallel::Parallel<'a>,
}
impl Scheduler for Recompute<'_> {
    /// Resolve eval using this scheduler's evaluation and placement choices.
    async fn eval(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::eval(input)?;
        if self.graph.current().is_some_and(|parent| parent.is_eval()) {
            return self.graph.defer(&node).await;
        }
        self.graph
            .evaluate(
                &node,
                parallel::Parallel::eval_using(self, input, &mut |_| Ok(())),
            )
            .await
    }
    /// Resolve think using this scheduler's evaluation and placement choices.
    async fn think(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::think(input)?;
        self.graph
            .evaluate(&node, collector::think(self, input, &mut |_| Ok(())))
            .await
    }
    /// Resolve force using this scheduler's evaluation and placement choices.
    async fn force(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::force(input)?;
        self.graph
            .evaluate(&node, collector::force(self, input, &mut |_| Ok(())))
            .await
    }
    /// Resolve apply using this scheduler's evaluation and placement choices.
    async fn apply(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::apply(input)?;
        if self.graph.current().is_some() {
            return self.graph.defer(&node).await;
        }
        self.graph
            .evaluate(&node, collector::apply(self, input, &mut |_| Ok(())))
            .await
    }
    /// Resolve select using this scheduler's evaluation and placement choices.
    async fn select(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::select(input)?;
        if self.graph.current().is_some() {
            return self.graph.defer(&node).await;
        }
        self.graph
            .evaluate(&node, collector::select(self, input, &mut |_| Ok(())))
            .await
    }

    /// Use Parallel's registered-source retrieval policy.
    async fn load(
        &self,
        value: &fixscheduler::shell::Value,
    ) -> Result<fixscheduler::shell::Value, Error> {
        self.parallel.load(value).await
    }
}
#[fixwasm::apply]
fn apply(combination: &Handle<Object<fixwasm::Tree>>) -> Result<fixscheduler::shell::Value, Error> {
    let graph = fixscheduler::executor::Graph::default();
    let policy = Recompute {
        graph: &graph,
        parallel: parallel::Parallel::new(&graph)?,
    };
    let io = fixscheduler::io::Io;
    let result = executor::run(combination, &policy, &graph, &io)?;
    let result = if !fixscheduler::kernel::worker::returns_coupon()? {
        fixscheduler::collector::result(&result)?
    } else {
        result
    };
    fixscheduler::kernel::worker::finish(&result)?;
    Ok(result)
}
