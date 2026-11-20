pub mod graph;
mod node;
pub mod scheduler;
pub mod task;
use crate::{
    shell::{Value, capture, child, fields},
    storage,
};

use fixwasm::{Error, Handle, Object, Tree, Value as AsValue, tree};
pub use graph::Graph;
use node::Node;
pub(crate) use node::describe_application;
pub use scheduler::Scheduler;

/// Poll a future, waking it when kernel operations complete.
pub fn block_on<F: core::future::Future>(
    future: F,
    io: &crate::io::Io,
) -> Result<F::Output, Error> {
    let mut task = task::Task::new(future);
    loop {
        let notification = io.listen()?;
        if let core::task::Poll::Ready(value) = task.poll() {
            return Ok(value);
        }
        if !task.is_ready() {
            notification.wait()?;
        }
    }
}

/// Poll evaluation tasks and start queued operations.
pub(crate) fn work<'a>(
    graph: &'a Graph,
    policy: &'a (impl Scheduler + ?Sized),
    tasks: &mut task::Tasks<'a, Result<(), Error>>,
) -> Result<bool, Error> {
    let (mut progress, completed) = tasks.poll();
    for result in completed {
        result?;
    }
    while let Some(node) = graph.take()? {
        tasks.spawn(graph.execute(node, policy));
        progress = true;
    }
    Ok(progress || tasks.is_ready())
}

/// Run evaluation futures and return the root result.
pub fn run(
    combination: &Handle<Object<Tree>>,
    policy: &(impl Scheduler + ?Sized),
    graph: &Graph,
    io: &crate::io::Io,
) -> Result<Value, Error> {
    assert_eq!(combination.len()?, 5);
    let mode = combination.get(2);
    let input = capture(combination.get(4))?;
    if mode.equals(fixwasm::Blob::create(b"serve"))? {
        return crate::remote::server::run(&input, policy, Some(graph), io);
    }
    if mode.equals(fixwasm::Blob::create(b"todo"))? {
        return todo(&input);
    }
    if mode.equals(fixwasm::Blob::create(b"load"))? {
        return serving(load(policy, &input), policy, io)?;
    }
    let node = operation(&capture(mode)?, &input)?;
    graph.submit(&node)?;
    let mut result = task::Task::new(graph.wait(&node));
    let mut tasks = task::Tasks::default();
    let mut requests = task::Tasks::default();
    loop {
        let notification = io.listen()?;
        let progress = work(graph, policy, &mut tasks)?
            | crate::remote::server::work(policy, Some(graph), &mut requests)?;
        if let core::task::Poll::Ready(value) = result.poll() {
            return value;
        }
        if !progress && !result.is_ready() {
            notification.wait()?;
        }
    }
}

/// Run an evaluator without allocating a dependency graph.
pub fn run_inline(
    combination: &Handle<Object<Tree>>,
    policy: &(impl Scheduler + ?Sized),
    io: &crate::io::Io,
) -> Result<Value, Error> {
    assert_eq!(combination.len()?, 5);
    let mode = combination.get(2);
    let input = capture(combination.get(4))?;
    if mode.equals(fixwasm::Blob::create(b"serve"))? {
        return crate::remote::server::run(&input, policy, None, io);
    }
    if mode.equals(fixwasm::Blob::create(b"todo"))? {
        return todo(&input);
    }
    if mode.equals(fixwasm::Blob::create(b"load"))? {
        return serving(load(policy, &input), policy, io)?;
    }
    let node = operation(&capture(mode)?, &input)?;
    serving(node.dispatch(policy), policy, io)?
}

fn serving<F: core::future::Future>(
    future: F,
    policy: &(impl Scheduler + ?Sized),
    io: &crate::io::Io,
) -> Result<F::Output, Error> {
    drive(future, policy, None, io)
}

pub(crate) fn drive<F: core::future::Future>(
    future: F,
    policy: &(impl Scheduler + ?Sized),
    graph: Option<&Graph>,
    io: &crate::io::Io,
) -> Result<F::Output, Error> {
    let mut task = task::Task::new(future);
    let mut requests = task::Tasks::default();
    let mut work = task::Tasks::default();
    loop {
        let notification = io.listen()?;
        let mut progress = crate::remote::server::work(policy, graph, &mut requests)?;
        if let Some(graph) = graph {
            progress |= self::work(graph, policy, &mut work)?;
        }
        if let core::task::Poll::Ready(value) = task.poll() {
            return Ok(value);
        }
        if !progress && !task.is_ready() {
            notification.wait()?;
        }
    }
}

pub(crate) fn operation(mode: &Value, input: &Value) -> Result<Node, Error> {
    if AsValue::equals(mode, fixwasm::Blob::create(b"eval"))? {
        Node::eval(input)
    } else if AsValue::equals(mode, fixwasm::Blob::create(b"think"))? {
        Node::think(input)
    } else if AsValue::equals(mode, fixwasm::Blob::create(b"force"))? {
        Node::force(input)
    } else if AsValue::equals(mode, fixwasm::Blob::create(b"apply"))? {
        Node::apply(input)
    } else if AsValue::equals(mode, fixwasm::Blob::create(b"select"))? {
        Node::select(input)
    } else {
        Err(Error::WrongType)
    }
}

pub(crate) fn todo(input: &Value) -> Result<Value, Error> {
    let action = child(input, 0)?;
    let value = child(input, 1)?;
    if AsValue::equals(&action, fixwasm::Blob::create(b"import"))? {
        match storage::import_source(&value)? {
            Some(value) => capture(tree![0u64, value]),
            None => capture(tree![1u64, fixwasm::Blob::create(b"could not import URI")]),
        }
    } else if AsValue::equals(&action, fixwasm::Blob::create(b"add"))? {
        storage::add_sources(&value, &fields(&child(input, 2)?)?)
    } else if AsValue::equals(&action, fixwasm::Blob::create(b"list"))? {
        storage::sources(&value)
    } else {
        Err(Error::WrongType)
    }
}

pub(crate) async fn load(
    policy: &(impl Scheduler + ?Sized),
    input: &Value,
) -> Result<Value, Error> {
    let mut pending = alloc::vec![(input.clone(), false)];
    let mut seen = alloc::collections::BTreeSet::new();
    while let Some((value, completed)) = pending.pop() {
        if completed {
            storage::put(&value)?;
            continue;
        }
        let mut name = storage::canonical_value(&value)?.raw_handle_bytes()?;
        name[31] &= 0xe0;
        if !seen.insert(name) {
            continue;
        }
        let object = policy.load(&value).await?;
        pending.push((object.clone(), true));
        if object.data_type()? == fixwasm::DataType::Tree {
            let tag = object.is_tag()?;
            pending.extend(
                fields(&object)?
                    .into_iter()
                    .enumerate()
                    .filter(|(index, _)| !tag || *index != 0)
                    .map(|(_, child)| (child, false)),
            );
        }
    }
    Ok(input.clone())
}
