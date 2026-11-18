use crate::parallel::Parallel;
use core::cell::RefCell;
use fixscheduler::executor::Graph;
use fixscheduler::executor::graph::Node;
use fixscheduler::{executor::Scheduler, kernel::Advice, remote::Cluster, shell::Value};
use fixwasm::Error;

pub struct Distributed<'a> {
    graph: &'a Graph,
    parallel: Parallel<'a>,
    cluster: RefCell<Option<alloc::rc::Rc<Cluster>>>,
    io: &'a fixscheduler::io::Io,
}
impl Scheduler for Distributed<'_> {
    /// Delegate top-level Eval to a remote; use Parallel for local dependencies.
    async fn eval(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::eval(input)?;
        if self.graph.current().is_none() {
            self.schedule(&node).await
        } else {
            self.parallel.eval(input).await
        }
    }
    /// Delegate top-level Think to a remote; use Parallel for local dependencies.
    async fn think(&self, input: &Value) -> Result<Value, Error> {
        let node = Node::think(input)?;
        if self.graph.current().is_none() {
            self.schedule(&node).await
        } else {
            self.parallel.think(input).await
        }
    }
    /// Use Parallel for local force work.
    async fn force(&self, input: &Value) -> Result<Value, Error> {
        self.parallel.force(input).await
    }
    /// Use Parallel for local apply work.
    async fn apply(&self, input: &Value) -> Result<Value, Error> {
        self.parallel.apply(input).await
    }
    /// Use Parallel for local select work.
    async fn select(&self, input: &Value) -> Result<Value, Error> {
        self.parallel.select(input).await
    }

    /// Retrieve data using the composed scheduler's source order.
    async fn load(&self, value: &Value) -> Result<Value, Error> {
        self.parallel.load(value).await
    }
    /// Forward advisory notifications to the composed scheduler.
    fn on_advice(&self, advice: &Advice) -> Result<(), Error> {
        self.parallel.on_advice(advice)
    }
}

impl<'a> Distributed<'a> {
    /// Delegate Eval and Think requests to the first reachable remote.
    /// Delegation failure is fatal; other requests remain local.
    async fn schedule(&self, request: &Node) -> Result<Value, Error> {
        let existing = self.cluster.borrow().clone();
        let cluster = if let Some(cluster) = existing {
            cluster
        } else {
            let connected = alloc::rc::Rc::new(Cluster::connect(self.io, false).await?);
            self.cluster.borrow_mut().get_or_insert(connected).clone()
        };
        if fixscheduler::remote::configured()?
            .iter()
            .all(|remote| remote.address.starts_with("file://"))
        {
            return match request.code() {
                1 => self.parallel.eval(request.input()).await,
                4 => self.parallel.think(request.input()).await,
                _ => Err(Error::WrongType),
            };
        }
        let location = cluster
            .information()
            .iter()
            .position(|worker| worker.parallelism > 0)
            .unwrap_or_else(|| {
                panic!(
                    "{}",
                    cluster
                        .first_failure()
                        .unwrap_or("no reachable remote worker")
                )
            })
            + 1;
        let coupon = cluster
            .execute(location, request.code(), request.input())
            .await
            .unwrap_or_else(|error| panic!("remote execution failed: {error:?}"))
            .expect("remote worker is unavailable");
        self.parallel
            .retain(&fixwasm::backend::Tree::try_from(coupon.clone())?)?;
        Ok(coupon)
    }
}

/// Run this scheduler and retain the final coupon when requested by the caller.
pub fn run(combination: &fixwasm::Handle<fixwasm::Object<fixwasm::Tree>>) -> Result<Value, Error> {
    use fixscheduler::{executor, kernel::worker, storage};
    let graph = Graph::default();
    let io = fixscheduler::io::Io;
    let policy = Distributed {
        io: &io,
        graph: &graph,
        parallel: Parallel::new(&graph)?,
        cluster: RefCell::new(None),
    };
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
