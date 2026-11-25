use alloc::collections::BTreeSet;
use core::cell::RefCell;
use fixscheduler::collector;
use fixscheduler::executor::Graph;
use fixscheduler::{
    executor::{Scheduler, graph::Node},
    kernel::Advice,
    shell::{Value, ValueType, fields},
    storage,
};
use fixwasm::backend::Tree;
use fixwasm::{Error, Value as AsValue};

pub struct Parallel<'a> {
    graph: &'a Graph,
    requested: RefCell<BTreeSet<[u8; 32]>>,
    cluster: RefCell<Option<alloc::rc::Rc<fixscheduler::remote::Cluster>>>,
}

impl Scheduler for Parallel<'_> {
    /// Reuse an Eval coupon or queue dependency evaluation for any worker.
    async fn eval(&self, input: &Value) -> Result<Value, Error> {
        if let Some(coupon) = collector::claims(1, input)?.into_iter().next() {
            fixscheduler::kernel::cached(1, input)?;
            return Ok(coupon);
        }
        let node = Node::eval(input)?;
        if self.graph.current().is_some_and(|parent| parent.is_eval()) {
            return self.graph.defer(&node).await;
        }
        self.graph
            .evaluate(&node, async {
                Self::eval_using(self, input, &mut |coupon| self.retain(coupon)).await
            })
            .await
    }
    /// For think, reuse a coupon or evaluate with shared dependencies.
    async fn think(&self, input: &Value) -> Result<Value, Error> {
        if let Some(coupon) = collector::claims(4, input)?.into_iter().next() {
            fixscheduler::kernel::cached(4, input)?;
            return Ok(coupon);
        }
        let node = Node::think(input)?;
        self.graph
            .evaluate(&node, async {
                collector::think(self, input, &mut |coupon| self.retain(coupon)).await
            })
            .await
    }
    /// For force, reuse a coupon or evaluate with shared dependencies.
    async fn force(&self, input: &Value) -> Result<Value, Error> {
        if let Some(coupon) = collector::claims(3, input)?.into_iter().next() {
            fixscheduler::kernel::cached(3, input)?;
            return Ok(coupon);
        }
        let node = Node::force(input)?;
        self.graph
            .evaluate(&node, async {
                collector::force(self, input, &mut |coupon| self.retain(coupon)).await
            })
            .await
    }
    /// For apply, reuse a coupon or queue dependency work.
    async fn apply(&self, input: &Value) -> Result<Value, Error> {
        if let Some(coupon) = collector::claims(2, input)?.into_iter().next() {
            fixscheduler::kernel::cached(2, input)?;
            return Ok(coupon);
        }
        let node = Node::apply(input)?;
        if self.graph.current().is_some() {
            return self.graph.defer(&node).await;
        }
        self.graph
            .evaluate(&node, async {
                collector::apply(self, input, &mut |coupon| self.retain(coupon)).await
            })
            .await
    }
    /// For select, reuse a coupon or queue dependency work.
    async fn select(&self, input: &Value) -> Result<Value, Error> {
        if let Some(coupon) = collector::claims(6, input)?.into_iter().next() {
            fixscheduler::kernel::cached(6, input)?;
            return Ok(coupon);
        }
        let node = Node::select(input)?;
        if self.graph.current().is_some() {
            return self.graph.defer(&node).await;
        }
        self.graph
            .evaluate(&node, async {
                collector::select(self, input, &mut |coupon| self.retain(coupon)).await
            })
            .await
    }

    /// Identify data locally, then try its registered sources in order.
    async fn load(&self, value: &Value) -> Result<Value, Error> {
        if let Some(value) = storage::identify(value)? {
            return Ok(value);
        }
        for uri in fields(&storage::sources(value)?)? {
            if let Some(value) = storage::acquire_from(value, &uri)? {
                return Ok(value);
            }
        }
        let cluster = self.cluster().await?;
        match cluster.load_backing(value).await {
            Ok(Some(value)) => Ok(value),
            Ok(None) => Err(Error::OutOfBounds),
            Err(error) => Err(self.graph.fail(
                Error::OutOfBounds,
                alloc::format!("could not load data: {error}"),
            )),
        }
    }

    /// Remember canonicalization advice for an available target.
    fn on_advice(&self, advice: &Advice) -> Result<(), Error> {
        if advice.key_is("fix::canonicalize")? {
            let target = if matches!(
                advice.target.value_type(),
                ValueType::Thunk | ValueType::Ref | ValueType::Encode
            ) {
                storage::identify(&advice.target)?
            } else {
                Some(advice.target.clone())
            };
            if let Some(target) = target {
                self.requested
                    .borrow_mut()
                    .insert(target.raw_handle_bytes()?);
            }
        }
        Ok(())
    }
}

impl<'a> Parallel<'a> {
    /// Evaluate tree children concurrently using the supplied scheduler's overrides.
    pub async fn eval_using(
        policy: &(impl Scheduler + ?Sized),
        input: &Value,
        retain: &mut dyn FnMut(&Tree) -> Result<(), Error>,
    ) -> Result<Value, Error> {
        if input.value_type() == ValueType::Encode || input.is_value()? {
            return alloc::boxed::Box::pin(collector::eval(policy, input, retain)).await;
        }
        let data = policy.load(input).await?;
        let children = fields(&data)?;
        let evaluation = fixscheduler::progress::evaluation(1, input);
        let result = async {
            let coupons = fixscheduler::executor::task::try_join_all(
                children
                    .iter()
                    .map(|child| alloc::boxed::Box::pin(policy.eval(child))),
            )
            .await?;
            collector::eval_tree(policy, input, &coupons, retain).await
        }
        .await;
        evaluation.finish(result)
    }

    /// Reuse the connections and inventories of the configured remote machines.
    pub(crate) async fn cluster(
        &self,
    ) -> Result<alloc::rc::Rc<fixscheduler::remote::Cluster>, Error> {
        if let Some(cluster) = self.cluster.borrow().clone() {
            return Ok(cluster);
        }
        let cluster = alloc::rc::Rc::new(
            fixscheduler::remote::Cluster::connect(&fixscheduler::io::Io, true).await?,
        );
        Ok(self.cluster.borrow_mut().get_or_insert(cluster).clone())
    }

    /// Cache coupons, retaining results for the advised input and its Thunk/Encode forms.
    pub fn retain(&self, coupon: &Tree) -> Result<(), Error> {
        let claim = storage::identify(&fixscheduler::shell::child(coupon, 1)?)?
            .ok_or(Error::OutOfBounds)?;
        let input = fixscheduler::shell::child(&claim, 1)?;
        let input = if matches!(
            input.value_type(),
            ValueType::Thunk | ValueType::Ref | ValueType::Encode
        ) {
            storage::identify(&input)?
        } else {
            Some(input)
        };
        let requested = match input {
            Some(input) => self.requested.borrow().contains(&input.raw_handle_bytes()?),
            None => false,
        };
        if requested {
            storage::recommend(coupon)?;
        } else {
            storage::cache(coupon, false)?;
        }
        Ok(())
    }

    /// Share local evaluation through the supplied dependency graph.
    pub fn new(graph: &'a Graph) -> Result<Self, Error> {
        Ok(Self {
            graph,
            requested: RefCell::default(),
            cluster: RefCell::new(None),
        })
    }
}

/// Run this scheduler and retain the final coupon when requested by the caller.
pub fn run(combination: &fixwasm::Handle<fixwasm::Object<fixwasm::Tree>>) -> Result<Value, Error> {
    use fixscheduler::{executor, kernel::worker, storage};
    let graph = Graph::default();
    let policy = Parallel::new(&graph)?;
    let io = fixscheduler::io::Io;
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
