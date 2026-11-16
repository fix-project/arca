use super::{Connection, bytes, certify, export, import, missing, protocol, trusted_keys};
use crate::{
    collector,
    executor::{Graph, Scheduler, graph::Node, task::Task},
    io::{Io, log},
    shell::{Value, child},
};
use alloc::{string::String, vec::Vec};
use core::task::Poll;
use fixwasm::{Blob, Error, Value as AsValue, tree};

const MAX_TRANSFER: usize = 512 << 20;

async fn accept(io: &Io, listener: u64) -> Result<Connection<'_>, Error> {
    let (status, stream) = io
        .request(tree![Blob::create(b"tcp_accept"), listener])
        .await?;
    if status != 0 {
        return Err(Error::OutOfBounds);
    }
    Ok(Connection(stream.read_u64()?, io))
}

async fn connection(
    mut stream: Connection<'_>,
    memory: u64,
    keys: &[String],
    policy: &(impl Scheduler + ?Sized),
    graph: Option<&Graph>,
) -> Result<(), Error> {
    loop {
        let information = super::information(memory)?;
        let Some(incoming) = protocol::asynchronous::receive_transfer_with_info(
            &mut stream,
            MAX_TRANSFER,
            &information,
            keys,
            |offers| missing(offers).ok().flatten(),
        )
        .await
        .map_err(|_| Error::WrongType)?
        else {
            return Ok(());
        };
        if incoming.result.is_some() {
            return Err(Error::WrongType);
        }
        let encoded = incoming.transfer.encode().map_err(|_| Error::WrongType)?;
        let roots = import(&encoded)?.ok_or(Error::WrongType)?;
        let [requested_collector, input] = roots.as_slice() else {
            return Err(Error::WrongType);
        };
        let collector = collector::callable()?;
        if !requested_collector.equals(&collector)? {
            return Err(Error::WrongType);
        }
        let node = match incoming.task.operation {
            1 => Node::eval(input)?,
            2 => Node::apply(input)?,
            3 => Node::force(input)?,
            4 => Node::think(input)?,
            6 => Node::select(input)?,
            _ => return Err(Error::WrongType),
        };
        let result = if let Some(graph) = graph {
            graph.defer(&node).await?
        } else {
            node.dispatch(policy).await?
        };
        let result =
            collector::validate(result, incoming.task.operation, input)?.ok_or(Error::WrongType)?;
        let encoded = export(core::slice::from_ref(&result), &[])?.ok_or(Error::WrongType)?;
        let transfer = protocol::Transfer::decode(&encoded).map_err(|_| Error::WrongType)?;
        let [coupon] = transfer.roots.as_slice() else {
            return Err(Error::WrongType);
        };
        protocol::asynchronous::send_transfer(
            &mut stream,
            MAX_TRANSFER,
            &incoming.task,
            Some(*coupon),
            transfer,
            |name, keys| certify(name, keys).ok().flatten(),
        )
        .await
        .map_err(|_| Error::WrongType)?;
    }
}

/// Serve connections while polling evaluation tasks.
/// An idle connection suspends its I/O future without occupying an evaluation worker.
pub fn run(
    input: &Value,
    policy: &(impl Scheduler + ?Sized),
    graph: Option<&Graph>,
    io: &Io,
) -> Result<Value, Error> {
    let memory = child(input, 2)?.read_u64()?;
    let keys = trusted_keys()?;
    let listener = {
        let (status, value) = crate::abi::request(tree![
            Blob::create(b"tcp_listen"),
            child(input, 0)?,
            child(input, 1)?
        ])?;
        if status != 0 {
            return Err(Error::OutOfBounds);
        }
        let id = value.read_u64()?;
        let (status, address) = crate::abi::request(tree![Blob::create(b"tcp_local_addr"), id])?;
        if status != 0 {
            return Err(Error::OutOfBounds);
        }
        let ip = bytes(&child(&address, 0)?)?;
        let port = child(&address, 1)?.read_u64()?;
        log(&alloc::format!(
            "listening on {}.{}.{}.{}:{}",
            ip[0],
            ip[1],
            ip[2],
            ip[3],
            port
        ))?;
        id
    };
    let mut accepting = Task::new(accept(io, listener));
    let mut connections = Vec::new();
    let mut work = crate::executor::task::Tasks::default();
    loop {
        let notification = io.listen()?;
        let mut progress = false;
        if let Poll::Ready(stream) = accepting.poll() {
            connections.push(Task::new(connection(stream?, memory, &keys, policy, graph)));
            accepting = Task::new(accept(io, listener));
            progress = true;
        }
        let mut index = 0;
        while index < connections.len() {
            if let Poll::Ready(result) = connections[index].poll() {
                if let Err(error) = result {
                    log(&alloc::format!("remote request failed: {error:?}"))?;
                }
                drop(connections.swap_remove(index));
                progress = true;
            } else {
                index += 1;
            }
        }
        if let Some(graph) = graph {
            progress |= crate::executor::work(graph, policy, &mut work)?;
        }
        if !progress && !accepting.is_ready() && !connections.iter().any(Task::is_ready) {
            notification.wait()?;
        }
    }
}
