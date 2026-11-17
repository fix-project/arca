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
    allow_updates: bool,
    policy: &(impl Scheduler + ?Sized),
    graph: Option<&Graph>,
) -> Result<Option<Value>, ConnectionError> {
    loop {
        let Some(first) =
            protocol::asynchronous::receive(&mut stream, protocol::MAX_MESSAGE_SIZE).await?
        else {
            return Ok(None);
        };
        match first {
            protocol::Message::RequestLabels => {
                protocol::asynchronous::send(
                    &mut stream,
                    protocol::Message::Labels(super::labels()?),
                )
                .await?;
                continue;
            }
            protocol::Message::RequestTrustedKeys => {
                protocol::asynchronous::send(
                    &mut stream,
                    protocol::Message::TrustedKeys(keys.to_vec()),
                )
                .await?;
                continue;
            }
            protocol::Message::RequestObject {
                label,
                name,
                trusted_keys,
                backing,
            } => {
                let reply = match super::export_label(&label, name, backing) {
                    Ok(transfer) if transfer.roots == [name] => {
                        match protocol::prepare_transfer(transfer, &trusted_keys, certify) {
                            Ok(transfer) => protocol::Message::ObjectTransfer(transfer),
                            Err(error) => protocol::Message::Failure(error),
                        }
                    }
                    Ok(_) => protocol::Message::Failure(String::from(
                        "exported value differs from the requested ref",
                    )),
                    Err(error) => protocol::Message::Failure(error),
                };
                protocol::asynchronous::send(&mut stream, reply).await?;
                continue;
            }
            protocol::Message::SubmitObject(transfer) if allow_updates => {
                let name = transfer.roots.as_slice();
                let reply = if let [name] = name {
                    let encoded = transfer.encode().map_err(|_| Error::WrongType)?;
                    match import(&encoded) {
                        Ok(roots) if roots.len() == 1 && roots[0].raw_handle_bytes()? == *name => {
                            protocol::Message::ObjectReceived(*name)
                        }
                        Ok(_) => protocol::Message::Failure(String::from(
                            "imported value differs from submitted root",
                        )),
                        Err(error) => protocol::Message::Failure(error),
                    }
                } else {
                    protocol::Message::Rejected
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
            protocol::Message::SubmitObject(_) | protocol::Message::UpdateLabel { .. } => {
                protocol::asynchronous::send(&mut stream, protocol::Message::Rejected).await?;
                continue;
            }
            _ => (),
        }
        let information = match &first {
            protocol::Message::RequestInfo { inventory: true } => super::information(memory)?,
            _ => protocol::Information {
                parallelism: crate::kernel::worker::count()?
                    .try_into()
                    .map_err(|_| Error::OutOfBounds)?,
                memory_bytes: Some(memory),
                ..protocol::Information::default()
            },
        };
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
        if incoming.task.is_stage_programs() && !allow_updates {
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
        if incoming.task.is_stage_programs() {
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
        let node = match incoming.task.operation {
            1 => Node::eval(input)?,
            2 => Node::apply(input)?,
            3 => Node::force(input)?,
            4 => Node::think(input)?,
            6 => Node::select(input)?,
            _ => return Err(Error::WrongType.into()),
        };
        let result = if let Some(graph) = graph {
            graph.defer(&node).await?
        } else {
            node.dispatch(policy).await?
        };
        let result =
            collector::validate(result, incoming.task.operation, input)?.ok_or(Error::WrongType)?;
        let encoded =
            export(core::slice::from_ref(&result), &[]).map_err(ConnectionError::Operation)?;
        let transfer = protocol::Transfer::decode(&encoded).map_err(|_| Error::WrongType)?;
        let [coupon] = transfer.roots.as_slice() else {
            return Err(Error::WrongType.into());
        };
        protocol::asynchronous::send_transfer(
            &mut stream,
            protocol::MAX_MESSAGE_SIZE,
            &incoming.task,
            Some(*coupon),
            transfer,
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
            Connection(stream.read_u64()?, io),
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
                    Err(error) => log(&alloc::format!("remote request failed: {error}"))?,
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
