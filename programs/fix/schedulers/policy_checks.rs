#![no_std]
extern crate alloc;
use fixscheduler::collector;
pub mod hint;
pub mod onepass;
pub mod parallel;
pub mod serial;
#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

use alloc::collections::BTreeMap;
use fixscheduler::{executor::graph::Node, remote::protocol::Information, shell::capture};
use fixwasm::{Error, Value as AsValue};
use onepass::Job;

fn request(id: u64) -> Result<Node, Error> {
    Node::think(&capture(id)?)
}
fn worker(parallelism: u32) -> Information {
    Information {
        parallelism,
        memory_bytes: Some(1024),
        ..Information::default()
    }
}

#[fixwasm::apply]
fn apply(
    combination: &fixwasm::Handle<fixwasm::Object<fixwasm::Tree>>,
) -> Result<fixscheduler::shell::Value, Error> {
    locality_and_cache()?;
    shared_dependencies()?;
    reconsider_parent()?;
    round_robin()?;
    scheduler_can_follow_dependency_callbacks()?;
    let graph = fixscheduler::executor::Graph::default();
    let io = fixscheduler::io::Io;
    cancelled_applications(&capture(combination.get(2))?, &io)?;
    fixscheduler::executor::block_on(
        async {
            advisory_notifications(&capture(combination.get(2))?).await?;
            serial_executes_and_parallel_defers(&graph, &capture(combination.get(2))?).await?;
            default_steps_call_overrides(&graph, &capture(combination.get(2))?).await?;
            parallel_children_are_shared_tasks(&graph)?;
            cancellation_preserves_shared_work(&graph)
        },
        &io,
    )??;
    capture(fixwasm::Blob::create(b"ok"))
}

fn locality_and_cache() -> Result<(), Error> {
    let request = request(0)?;
    let mut jobs = BTreeMap::from([(
        request.clone(),
        Job {
            present: [(1, 100)].into(),
            output_bytes: 200,
            ..Job::default()
        },
    )]);
    let workers = [worker(1), worker(4)];
    let mut assignments = BTreeMap::new();
    onepass::assign(&request, &jobs, &workers, &mut assignments);
    assert_eq!(assignments[&request], (1, 100));
    hint::pull_back_outputs(&request, &jobs, &workers, &mut assignments);
    assert_eq!(assignments[&request], (0, 100));
    jobs.get_mut(&request).unwrap().memory_bytes = Some(2048);
    onepass::assign(&request, &jobs, &workers, &mut assignments);
    assert_eq!(assignments[&request], (0, 0));
    let job = jobs.get_mut(&request).unwrap();
    job.cached.insert(1);
    job.parallel = true;
    onepass::assign(&request, &jobs, &workers, &mut assignments);
    assert_eq!(assignments[&request], (1, 0));
    let job = jobs.get_mut(&request).unwrap();
    job.cached.clear();
    job.parallel = false;
    job.memory_bytes = None;
    job.present.clear();
    onepass::assign(&request, &jobs, &workers, &mut assignments);
    assert_eq!(assignments[&request], (0, 0));
    Ok(())
}

fn shared_dependencies() -> Result<(), Error> {
    let root = request(0)?;
    let left = request(1)?;
    let right = request(2)?;
    let shared = request(3)?;
    let jobs = BTreeMap::from([
        (
            root.clone(),
            Job {
                dependencies: [left.clone(), right.clone()].into(),
                ..Job::default()
            },
        ),
        (
            left.clone(),
            Job {
                dependencies: [shared.clone()].into(),
                output_bytes: 100,
                ..Job::default()
            },
        ),
        (
            right.clone(),
            Job {
                dependencies: [shared.clone()].into(),
                output_bytes: 100,
                ..Job::default()
            },
        ),
        (
            shared.clone(),
            Job {
                present: [(1, 10)].into(),
                output_bytes: 100,
                ..Job::default()
            },
        ),
    ]);
    let mut assignments = BTreeMap::new();
    onepass::assign(&root, &jobs, &[worker(1), worker(1)], &mut assignments);
    assert_eq!(assignments[&shared], (1, 10));
    assert_eq!(assignments[&left], (1, 100));
    assert_eq!(assignments[&right], (1, 100));
    assert_eq!(assignments[&root], (1, 200));
    Ok(())
}

fn reconsider_parent() -> Result<(), Error> {
    let root = request(0)?;
    let child = request(1)?;
    let jobs = BTreeMap::from([
        (
            root.clone(),
            Job {
                dependencies: [child.clone()].into(),
                output_bytes: 10,
                ..Job::default()
            },
        ),
        (
            child.clone(),
            Job {
                output_bytes: 1,
                ..Job::default()
            },
        ),
    ]);
    let mut assignments = BTreeMap::from([(root.clone(), (1, 10)), (child.clone(), (1, 20))]);
    hint::pull_back_outputs(&root, &jobs, &[worker(1), worker(1)], &mut assignments);
    assert_eq!(assignments[&root], (1, 20));
    assert_eq!(assignments[&child], (1, 20));
    Ok(())
}

fn round_robin() -> Result<(), Error> {
    let root = request(0)?;
    let mut jobs = BTreeMap::new();
    let mut assignments = BTreeMap::from([(root.clone(), (0, 0))]);
    let mut dependencies = alloc::collections::BTreeSet::new();
    for index in 1..9 {
        let child = request(index)?;
        dependencies.insert(child.clone());
        assignments.insert(child.clone(), (0, index));
        jobs.insert(child, Job::default());
    }
    jobs.insert(
        root.clone(),
        Job {
            dependencies,
            ..Job::default()
        },
    );
    hint::spread_independent(
        &root,
        &jobs,
        &[worker(1), worker(1), worker(8), worker(100)],
        &mut assignments,
    );
    for index in 1..7 {
        assert_eq!(
            assignments[&request(index)?],
            (if index % 2 == 1 { 1 } else { 2 }, index)
        );
    }
    for index in 7..9 {
        assert_eq!(assignments[&request(index)?], (0, index));
    }
    Ok(())
}

fn scheduler_can_follow_dependency_callbacks() -> Result<(), Error> {
    use alloc::collections::BTreeSet;
    use core::{
        cell::RefCell,
        future::Future,
        task::{Context, Waker},
    };
    use fixscheduler::{
        executor::{Graph, Scheduler},
        shell::Value,
    };
    use fixwasm::tree;
    struct Lookahead {
        graph: Graph,
        visited: RefCell<BTreeSet<Node>>,
        edges: RefCell<BTreeSet<(Node, Node)>>,
    }
    impl Scheduler for Lookahead {
        async fn eval(&self, input: &Value) -> Result<Value, Error> {
            self.visit(
                &Node::eval(input)?,
                alloc::boxed::Box::pin(self.default_eval(input)),
            )
            .await
        }
        async fn think(&self, input: &Value) -> Result<Value, Error> {
            self.visit(
                &Node::think(input)?,
                alloc::boxed::Box::pin(self.default_think(input)),
            )
            .await
        }
        async fn force(&self, input: &Value) -> Result<Value, Error> {
            self.visit(
                &Node::force(input)?,
                alloc::boxed::Box::pin(self.default_force(input)),
            )
            .await
        }
        async fn apply(&self, input: &Value) -> Result<Value, Error> {
            self.visit(&Node::apply(input)?, self.default_apply(input))
                .await
        }
        async fn select(&self, input: &Value) -> Result<Value, Error> {
            self.visit(&Node::select(input)?, self.default_select(input))
                .await
        }
    }
    impl Lookahead {
        async fn visit(
            &self,
            request: &Node,
            evaluate: impl Future<Output = Result<Value, Error>>,
        ) -> Result<Value, Error> {
            if let Some(parent) = self.graph.current() {
                self.edges.borrow_mut().insert((parent, request.clone()));
            }
            let inserted = self.visited.borrow_mut().insert(request.clone());
            if inserted
                && !(request.is_apply() || request.is_select())
                && !(request.is_eval() && request.input().is_value()?)
            {
                self.graph.within(request, evaluate).await?;
            }
            core::future::pending().await
        }
    }
    let policy = Lookahead {
        graph: Graph::default(),
        visited: RefCell::default(),
        edges: RefCell::default(),
    };
    let object = capture(tree![7u64])?;
    let thunk = capture(object.application())?;
    let encode = capture(thunk.strict())?;
    let input = capture(tree![&encode, &encode])?;
    let request = Node::eval(&input)?;
    {
        let mut future =
            alloc::boxed::Box::pin(policy.graph.within(&request, policy.default_eval(&input)));
        assert!(
            future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
    }
    let visited = policy.visited.borrow();
    assert!(visited.contains(&Node::eval(&encode)?));
    assert!(visited.contains(&Node::force(&thunk)?));
    assert!(visited.contains(&Node::think(&thunk)?));
    assert!(visited.contains(&Node::eval(&object)?));
    let edges = policy.edges.borrow();
    let evaluated = Node::eval(&encode)?;
    let forced = Node::force(&thunk)?;
    let thought = Node::think(&thunk)?;
    assert_eq!(edges.len(), 4);
    assert!(edges.contains(&(request, evaluated.clone())));
    assert!(edges.contains(&(evaluated, forced.clone())));
    assert!(edges.contains(&(forced, thought.clone())));
    assert!(edges.contains(&(thought, Node::eval(&object)?)));
    assert!(policy.graph.current().is_none());
    Ok(())
}

fn cancelled_applications(
    combination: &fixscheduler::shell::Value,
    io: &fixscheduler::io::Io,
) -> Result<(), Error> {
    use fixscheduler::{
        executor::{block_on, task::Task},
        kernel,
    };
    for delivered in [false, true] {
        let mut on_advice = |_: &kernel::Advice| Ok(());
        let mut cancelled = Task::new(kernel::run(combination, &mut on_advice));
        assert!(cancelled.poll().is_pending());
        if delivered {
            while !cancelled.is_ready() {
                let notification = io.listen()?;
                if !cancelled.is_ready() {
                    notification.wait()?;
                }
            }
        }
        drop(cancelled);
        let result = block_on(kernel::run(combination, &mut |_| Ok(())), io)??;
        assert!(result.equals(combination)?);
    }
    Ok(())
}

async fn advisory_notifications(combination: &fixscheduler::shell::Value) -> Result<(), Error> {
    use fixscheduler::{kernel, shell::child};
    let initial = [0u64.to_le_bytes(), 2u64.to_le_bytes()].concat();
    let completed = [2u64.to_le_bytes(), 2u64.to_le_bytes()].concat();
    let expected: &[(&str, &[u8])] = &[
        ("fix::title", b"probe"),
        ("fix::progress", &initial),
        ("fix::progress", &completed),
        ("sample::binary", &[0, 255]),
        ("sample::local", b"probe"),
        ("sample::local", b"probe"),
    ];
    let mut index = 0;
    let coupon = kernel::apply(combination, &mut |event| {
        let (key, payload) = expected[index];
        assert!(event.key_is(key)?);
        assert!(AsValue::equals(
            &event.payload,
            fixwasm::Blob::create(payload)
        )?);
        assert!(fixscheduler::storage::identify(&event.target)?.is_some());
        index += 1;
        Ok(())
    })
    .await?;
    assert_eq!(index, expected.len());
    assert!(coupon.is_tag()?);
    let claim = fixscheduler::storage::identify(&child(&coupon, 1)?)?.unwrap();
    assert!(child(&claim, 2)?.equals(combination)?);
    Ok(())
}

async fn serial_executes_and_parallel_defers(
    graph: &fixscheduler::executor::Graph,
    combination: &fixscheduler::shell::Value,
) -> Result<(), Error> {
    use core::{
        future::Future,
        task::{Context, Waker},
    };
    use fixscheduler::{
        executor::Scheduler,
        shell::{Value, fields},
    };
    use fixwasm::{Blob, Tree, tree};
    struct Probe<'a, S> {
        scheduler: &'a S,
    }
    impl<S: Scheduler> Scheduler for Probe<'_, S> {
        async fn eval(&self, input: &Value) -> Result<Value, Error> {
            self.scheduler.apply(input).await?;
            core::future::pending().await
        }
    }
    let serial = serial::Serial::new()?;
    serial.apply(combination).await?;
    let coupon = collector::claims(2, combination)?
        .into_iter()
        .next()
        .unwrap();
    assert!(collector::validate(coupon.clone(), 2, combination)?.is_some());
    assert!(collector::validate(coupon.clone(), 1, combination)?.is_none());
    assert!(
        collector::validate(
            coupon.clone(),
            2,
            &capture(Blob::create(b"different input"))?
        )?
        .is_none()
    );
    assert!(collector::validate(capture(tree![])?, 2, combination)?.is_none());
    assert!(collector::result(&coupon)?.equals(combination)?);
    assert!(graph.take()?.is_none());
    let mut arguments = fields(combination)?;
    arguments.push(capture(Blob::create(b"parallel"))?);
    let combination = capture(Tree::from_iter(&arguments))?;
    let request = Node::apply(&combination)?;
    let parallel = parallel::Parallel::new(graph)?;
    let thunk = capture(combination.application())?;
    {
        let parent = Node::think(&thunk)?;
        let probe = Probe {
            scheduler: &parallel,
        };
        let mut future = alloc::boxed::Box::pin(graph.within(&parent, probe.default_think(&thunk)));
        assert!(
            future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
    }
    assert!(graph.completed(&request)?.is_none());
    let ready = graph.take()?.unwrap();
    assert!(ready == request);
    graph.execute(ready, &parallel).await?;
    assert!(collector::result(&parallel.apply(&combination).await?)?.equals(&combination)?);
    Ok(())
}

async fn default_steps_call_overrides(
    graph: &fixscheduler::executor::Graph,
    combination: &fixscheduler::shell::Value,
) -> Result<(), Error> {
    use core::cell::Cell;
    use fixscheduler::{executor::Scheduler, shell::Value};
    use fixwasm::tree;
    #[derive(Default)]
    struct CountApplications(Cell<usize>);
    impl Scheduler for CountApplications {
        async fn apply(&self, input: &Value) -> Result<Value, Error> {
            self.0.set(self.0.get() + 1);
            self.default_apply(input).await
        }
    }
    let scheduler = CountApplications::default();
    let thunk = capture(combination.application())?;
    let encode = capture(thunk.strict())?;
    let input = capture(tree![&encode, &encode])?;
    let result = scheduler.eval(&input).await?;
    assert!(result.equals(&capture(tree![combination, combination])?)?);
    assert_eq!(scheduler.0.get(), 2);
    let selection = capture(tree![&input, 1u64])?;
    let selected = capture(selection.selection().strict())?;
    assert!(scheduler.eval(&selected).await?.equals(combination)?);
    assert_eq!(scheduler.0.get(), 4);
    let shallow = capture(thunk.shallow())?;
    let reference = scheduler.eval(&shallow).await?;
    assert!(reference.equals(&fixscheduler::shell::reference(combination)?)?);
    let request = Node::eval(&capture(fixwasm::Blob::create(b"opaque result"))?)?;
    assert!(matches!(
        graph.claim(&request)?,
        fixscheduler::executor::graph::Claim::Owned
    ));
    let value = scheduler.eval(request.input()).await?;
    graph.complete(&request, &value)?;
    assert!(graph.completed(&request)?.unwrap().equals(&value)?);
    Ok(())
}

fn cancellation_preserves_shared_work(graph: &fixscheduler::executor::Graph) -> Result<(), Error> {
    use fixscheduler::{
        executor::{Scheduler, task::Task},
        shell::Value,
    };
    struct Suspends;
    impl Scheduler for Suspends {
        async fn eval(&self, _: &Value) -> Result<Value, Error> {
            core::future::pending().await
        }
    }
    let request = Node::eval(&capture(fixwasm::Blob::create(b"shared waiter"))?)?;
    graph.submit(&request)?;
    let mut first = Task::new(graph.wait(&request));
    let mut second = Task::new(graph.wait(&request));
    assert!(first.poll().is_pending());
    assert!(second.poll().is_pending());
    drop(first);
    let ready = graph.take()?.unwrap();
    assert!(ready == request);
    let mut owner = Task::new(graph.execute(ready, &Suspends));
    assert!(owner.poll().is_pending());
    assert!(graph.take()?.is_none());
    drop(owner);
    let ready = graph.take()?.unwrap();
    assert!(ready == request);
    let result = capture(42u64)?;
    graph.complete(&ready, &result)?;
    assert!(matches!(second.poll(), core::task::Poll::Ready(Ok(value)) if value == result));
    Ok(())
}

fn parallel_children_are_shared_tasks(graph: &fixscheduler::executor::Graph) -> Result<(), Error> {
    use core::task::Poll;
    use fixscheduler::executor::task::{Task, Tasks};
    use fixwasm::{Blob, tree};

    let parallel = parallel::Parallel::new(graph)?;
    let first = Blob::create(b"first independent evaluation");
    let second = Blob::create(b"second independent evaluation");
    let children = [
        capture(first.identification().strict())?,
        capture(second.identification().strict())?,
    ];
    let input = capture(tree![&children[0], &children[1]])?;
    let root = Node::eval(&input)?;
    let mut retain = |coupon: &fixwasm::backend::Tree| parallel.retain(coupon);
    let mut result = Task::new(graph.within(
        &root,
        parallel::Parallel::eval_using(&parallel, &input, &mut retain),
    ));
    assert!(result.poll().is_pending());
    let mut tasks = Tasks::default();
    for child in &children {
        let queued = graph.take()?.unwrap();
        assert!(queued == Node::eval(child)?);
        tasks.spawn(graph.execute(queued, &parallel));
    }
    assert!(graph.take()?.is_none());
    loop {
        let notification = fixscheduler::kernel::worker::listen()?;
        fixscheduler::io::Io.wake_completed()?;
        for completion in tasks.poll().1 {
            completion?;
        }
        if let Some(queued) = graph.take()? {
            tasks.spawn(graph.execute(queued, &parallel));
        }
        if let Poll::Ready(coupon) = result.poll() {
            assert!(collector::result(&coupon?)?.equals(&capture(tree![first, second])?)?);
            return Ok(());
        }
        if !tasks.is_ready() && !result.is_ready() {
            notification.wait()?;
        }
    }
}
