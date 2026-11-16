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

/// Poll evaluation tasks and start another queued operation when available.
pub(crate) fn work<'a>(
    graph: &'a Graph,
    policy: &'a (impl Scheduler + ?Sized),
    tasks: &mut task::Tasks<'a, Result<(), Error>>,
) -> Result<bool, Error> {
    let (mut progress, completed) = tasks.poll();
    for result in completed {
        result?;
    }
    if let Some(node) = graph.take()? {
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
    let node = operation(&capture(mode)?, &input)?;
    graph.submit(&node)?;
    let mut result = task::Task::new(graph.wait(&node));
    let mut tasks = task::Tasks::default();
    loop {
        let notification = io.listen()?;
        let progress = work(graph, policy, &mut tasks)?;
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
    let node = operation(&capture(mode)?, &input)?;
    block_on(node.dispatch(policy), io)?
}

fn operation(mode: &Value, input: &Value) -> Result<Node, Error> {
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

fn todo(input: &Value) -> Result<Value, Error> {
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
