use super::{Connection, bytes, certify, import, missing, protocol, trusted_keys};
use crate::{
    collector,
    executor::{Graph, Scheduler, graph::Node, task::Task},
    io::{Io, log},
    shell::{Value, child},
};
use alloc::{string::String, vec::Vec};
use core::{
    future::{Future, poll_fn},
    task::Poll,
};
use fixwasm::{Blob, Error, Value as AsValue, tree};

fn as_bool(value: &Value) -> Result<bool, Error> {
    Ok(value.read_u64()? != 0)
}

#[derive(Debug)]
enum ConnectionError {
    Shell(Error),
    Protocol(protocol::Error<super::IoError>),
    Operation(String),
}

impl core::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Shell(error) => write!(f, "scheduler error: {error:?}"),
            Self::Protocol(error) => error.fmt(f),
            Self::Operation(error) => f.write_str(error),
        }
    }
}

impl From<Error> for ConnectionError {
    fn from(error: Error) -> Self {
        Self::Shell(error)
    }
}

impl From<protocol::Error<super::IoError>> for ConnectionError {
    fn from(error: protocol::Error<super::IoError>) -> Self {
        Self::Protocol(error)
    }
}

async fn accept(io: &Io, listener: u64) -> Result<Connection, Error> {
    let (status, stream) = io
        .request(tree![Blob::create(b"tcp_accept"), listener])
        .await?;
    if status != 0 {
        return Err(Error::OutOfBounds);
    }
    Ok(Connection(stream.read_u64()?))
}

fn capacity(memory: u64) -> Result<protocol::Information, Error> {
    Ok(protocol::Information {
        parallelism: crate::kernel::worker::count()?
            .try_into()
            .map_err(|_| Error::OutOfBounds)?,
        memory_bytes: Some(memory),
        ..protocol::Information::default()
    })
}

async fn connection(
    mut stream: Connection,
    memory: u64,
    keys: &[String],
    allow_updates: bool,
    policy: &(impl Scheduler + ?Sized),
    graph: Option<&Graph>,
) -> Result<Option<Value>, ConnectionError> {
    loop {
        let first =
            match protocol::asynchronous::receive(&mut stream, protocol::MAX_MESSAGE_SIZE).await {
                Ok(Some(message)) => message,
                Ok(None) => return Ok(None),
                Err(error) => {
                    let _ = protocol::asynchronous::send(
                        &mut stream,
                        protocol::Message::Failure(alloc::format!(
                            "remote protocol {}: {error}",
                            protocol::VERSION
                        )),
                    )
                    .await;
                    return Err(error.into());
                }
            };
        match first {
            protocol::Message::RequestInfo { inventory } => {
                let information = if inventory {
                    super::information(memory)?
                } else {
                    capacity(memory)?
                };
                protocol::asynchronous::send(&mut stream, protocol::Message::Info(information))
                    .await?;
                continue;
            }
            protocol::Message::RequestProgress => {
                let snapshot = super::bytes(&crate::abi::effect(b"progress_status", &[])?)?;
                protocol::asynchronous::send(
                    &mut stream,
                    protocol::Message::EvaluationProgress(snapshot),
                )
                .await?;
                continue;
            }
            protocol::Message::RequestLabels => {
                let labels = crate::abi::effect(b"remote_labels", &[])?;
                let labels = crate::storage::put(&labels)?;
                let name = labels.raw_handle_bytes()?;
                let proposal =
                    super::propose_object(name, true).map_err(ConnectionError::Operation)?;
                protocol::asynchronous::send_proposal(
                    &mut stream,
                    protocol::MAX_MESSAGE_SIZE,
                    None,
                    Some(name),
                    proposal,
                    super::read_object,
                    |name, keys| certify(name, keys).ok(),
                )
                .await?;
                continue;
            }
            protocol::Message::RequestTransfer(name) => {
                let proposal = match super::propose_object(name, true) {
                    Ok(proposal) => proposal,
                    Err(error) => {
                        protocol::asynchronous::send(
                            &mut stream,
                            protocol::Message::Failure(error),
                        )
                        .await?;
                        continue;
                    }
                };
                protocol::asynchronous::send_proposal(
                    &mut stream,
                    protocol::MAX_MESSAGE_SIZE,
                    None,
                    Some(name),
                    proposal,
                    super::read_object,
                    |name, keys| certify(name, keys).ok(),
                )
                .await?;
                continue;
            }
            proposal @ protocol::Message::ProposeTransfer {
                task: None,
                result: Some(name),
                ..
            } if allow_updates => {
                let incoming = protocol::asynchronous::receive_transfer_with_info_from(
                    &mut stream,
                    protocol::MAX_MESSAGE_SIZE,
                    &capacity(memory)?,
                    keys,
                    |offers| missing(offers).ok().flatten(),
                    Some(proposal),
                )
                .await?
                .ok_or(protocol::Error::Invalid)?;
                let encoded = incoming.transfer.encode().map_err(|_| Error::WrongType)?;
                let reply = match import(&encoded) {
                    Ok(roots) if roots.len() == 1 && roots[0].raw_handle_bytes()? == name => {
                        protocol::Message::ObjectReceived(name)
                    }
                    Ok(_) => protocol::Message::Failure(String::from(
                        "imported value differs from proposed root",
                    )),
                    Err(error) => protocol::Message::Failure(error),
                };
                protocol::asynchronous::send(&mut stream, reply).await?;
                continue;
            }
            protocol::Message::UpdateLabel {
                label,
                expected,
                value,
            } if allow_updates => {
                let reply = match super::update_label(&label, expected, value) {
                    Ok(()) => protocol::Message::LabelUpdated,
                    Err(error) => protocol::Message::Failure(error),
                };
                protocol::asynchronous::send(&mut stream, reply).await?;
                continue;
            }
            protocol::Message::ProposeTransfer { task: None, .. }
            | protocol::Message::UpdateLabel { .. } => {
                protocol::asynchronous::send(&mut stream, protocol::Message::Rejected).await?;
                continue;
            }
            _ => (),
        }
        let information = capacity(memory)?;
        let incoming = protocol::asynchronous::receive_transfer_with_info_from(
            &mut stream,
            protocol::MAX_MESSAGE_SIZE,
            &information,
            keys,
            |offers| missing(offers).ok().flatten(),
            Some(first),
        )
        .await;
        let Some(incoming) = (match incoming {
            Ok(incoming) => incoming,
            Err(error) => {
                let _ = protocol::asynchronous::send(
                    &mut stream,
                    protocol::Message::Failure(alloc::format!("transfer rejected: {error}")),
                )
                .await;
                return Err(error.into());
            }
        }) else {
            return Ok(None);
        };
        if incoming.result.is_some() {
            return Err(Error::WrongType.into());
        }
        let task = incoming.task.as_ref().ok_or(protocol::Error::Invalid)?;
        if task.is_stage_programs() && !allow_updates {
            protocol::asynchronous::send(
                &mut stream,
                protocol::Message::Failure(String::from("program staging requires SSH")),
            )
            .await?;
            continue;
        }
        let encoded = incoming.transfer.encode().map_err(|_| Error::WrongType)?;
        let roots = match import(&encoded) {
            Ok(roots) => roots,
            Err(error) => {
                protocol::asynchronous::send(&mut stream, protocol::Message::Failure(error))
                    .await?;
                continue;
            }
        };
        let [requested_collector, input] = roots.as_slice() else {
            return Err(Error::WrongType.into());
        };
        if task.is_stage_programs() {
            if !AsValue::equals(requested_collector, Blob::create(b""))?
                && let Err(error) = super::trust_collector(requested_collector)
            {
                protocol::asynchronous::send(&mut stream, protocol::Message::Failure(error))
                    .await?;
                continue;
            }
            return Ok(Some(crate::shell::capture(tree![
                input,
                requested_collector
            ])?));
        }
        let collector = collector::callable()?;
        if !requested_collector.equals(&collector)? {
            protocol::asynchronous::send(
                &mut stream,
                protocol::Message::Failure(String::from(
                    "requested coupon collector is not the server's trusted collector",
                )),
            )
            .await?;
            continue;
        }
        let node = match task.operation {
            1 => Node::eval(input)?,
            2 => Node::apply(input)?,
            3 => Node::force(input)?,
            4 => Node::think(input)?,
            6 => Node::select(input)?,
            _ => return Err(Error::WrongType.into()),
        };
        let observer = super::ProgressObserver::new()?;
        let reporting = crate::progress::operation(task.operation, input);
        protocol::asynchronous::send(
            &mut stream,
            protocol::Message::EvaluationProgress(observer.snapshot()?),
        )
        .await?;
        let evaluation = async {
            let result = if let Some(graph) = graph {
                graph.defer(&node).await?
            } else {
                node.dispatch(policy).await?
            };
            collector::validate(result, task.operation, input)?.ok_or(Error::WrongType)
        };
        let mut evaluation = core::pin::pin!(evaluation);
        let result = loop {
            let tick = Io.request(tree![Blob::create(b"sleep"), 250u64]);
            let mut tick = core::pin::pin!(tick);
            let result = poll_fn(|cx| {
                if let Poll::Ready(result) = evaluation.as_mut().poll(cx) {
                    return Poll::Ready(Some(result));
                }
                if tick.as_mut().poll(cx).is_ready() {
                    return Poll::Ready(None);
                }
                Poll::Pending
            })
            .await;
            protocol::asynchronous::send(
                &mut stream,
                protocol::Message::EvaluationProgress(observer.snapshot()?),
            )
            .await?;
            if let Some(result) = result {
                let result = reporting.finish(result);
                protocol::asynchronous::send(
                    &mut stream,
                    protocol::Message::EvaluationProgress(observer.snapshot()?),
                )
                .await?;
                break result?;
            }
        };
        let result = crate::storage::put(&result)?;
        let name = result.raw_handle_bytes()?;
        let proposal = super::propose_object(name, true).map_err(ConnectionError::Operation)?;
        protocol::asynchronous::send_proposal(
            &mut stream,
            protocol::MAX_MESSAGE_SIZE,
            Some(task),
            Some(name),
            proposal,
            super::read_object,
            |name, keys| certify(name, keys).ok(),
        )
        .await?;
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
    let stdio = crate::shell::as_tree(input)?.len()? > 3 && as_bool(&child(input, 3)?)?;
    let listener = if stdio {
        None
    } else {
        Some({
            let (status, value) = crate::abi::request(tree![
                Blob::create(b"tcp_listen"),
                child(input, 0)?,
                child(input, 1)?
            ])?;
            if status != 0 {
                return Err(Error::OutOfBounds);
            }
            let id = value.read_u64()?;
            let (status, address) =
                crate::abi::request(tree![Blob::create(b"tcp_local_addr"), id])?;
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
        })
    };
    let mut accepting = listener.map(|listener| Task::new(accept(io, listener)));
    let mut connections = Vec::new();
    if stdio {
        let (status, stream) = crate::abi::request(tree![Blob::create(b"stdio_stream")])?;
        if status != 0 {
            return Err(Error::WrongType);
        }
        connections.push(Task::new(connection(
            Connection(stream.read_u64()?),
            memory,
            &keys,
            true,
            policy,
            graph,
        )));
    }
    let mut work = crate::executor::task::Tasks::default();
    loop {
        let notification = io.listen()?;
        let mut progress = false;
        if let (Some(accepting_task), Some(listener)) = (&mut accepting, listener)
            && let Poll::Ready(stream) = accepting_task.poll()
        {
            connections.push(Task::new(connection(
                stream?, memory, &keys, false, policy, graph,
            )));
            accepting = Some(Task::new(accept(io, listener)));
            progress = true;
        }
        let mut index = 0;
        while index < connections.len() {
            if let Poll::Ready(result) = connections[index].poll() {
                match result {
                    Ok(Some(programs)) => {
                        crate::kernel::worker::finish(&programs)?;
                        return Ok(programs);
                    }
                    Ok(None) => (),
                    Err(error) => log(&alloc::format!(
                        "remote request failed (protocol {}): {error}",
                        protocol::VERSION
                    ))?,
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
        if stdio && connections.is_empty() {
            return crate::shell::capture(tree![]);
        }
        if !progress
            && !accepting.as_ref().is_some_and(Task::is_ready)
            && !connections.iter().any(Task::is_ready)
        {
            notification.wait()?;
        }
    }
}
