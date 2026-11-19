use super::{Connection, bytes, certify, import, missing, protocol, trusted_keys};
use crate::{
    collector,
    executor::{Graph, Scheduler, graph::Node, task::Task},
    io::{Io, log},
    shell::{Value, child},
};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
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
    Ok(Connection::raw(stream.read_u64()?))
}

fn capacity(memory: u64) -> Result<protocol::Information, Error> {
    Ok(protocol::Information {
        parallelism: crate::kernel::worker::count()?
            .try_into()
            .map_err(|_| Error::OutOfBounds)?,
        memory_bytes: Some(memory.min(crate::kernel::worker::available_memory()?)),
        memory_capacity_bytes: Some(memory.min(crate::kernel::worker::memory_capacity()?)),
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
    let result = handle(&mut stream, memory, keys, allow_updates, policy, graph).await;
    if let Err(error) = &result
        && !matches!(
            error,
            ConnectionError::Protocol(protocol::Error::IncompleteTransfer)
        )
    {
        let _ = protocol::asynchronous::send(
            &mut stream,
            protocol::Message::Failure(alloc::format!("{error}")),
        )
        .await;
    }
    result
}

async fn handle(
    stream: &mut Connection,
    memory: u64,
    keys: &[String],
    allow_updates: bool,
    policy: &(impl Scheduler + ?Sized),
    graph: Option<&Graph>,
) -> Result<Option<Value>, ConnectionError> {
    let mut first_request = true;
    loop {
        if !first_request && matches!(stream, Connection::Request(_)) {
            return Ok(None);
        }
        first_request = false;
        let Some(first) =
            protocol::asynchronous::receive(stream, protocol::MAX_MESSAGE_SIZE).await?
        else {
            return Ok(None);
        };
        match first {
            protocol::Message::Hello { link, node } => {
                let Connection::Raw(raw) = stream else {
                    return Err(protocol::Error::Invalid.into());
                };
                super::network::Network::get()?
                    .accept(Connection::Raw(raw.clone()), link, node)
                    .await
                    .map_err(|_| Error::WrongType)?;
                return Ok(None);
            }
            protocol::Message::RequestInfo { inventory } => {
                let information = if inventory {
                    super::information(memory)?
                } else {
                    capacity(memory)?
                };
                protocol::asynchronous::send(stream, protocol::Message::Info(information)).await?;
                continue;
            }
            protocol::Message::RequestProgress => {
                let snapshot = match &*stream {
                    Connection::Request(request) => request.progress()?,
                    Connection::Raw(_) => {
                        super::bytes(&crate::abi::effect(b"progress_status", &[])?)?
                    }
                };
                protocol::asynchronous::send(
                    stream,
                    protocol::Message::EvaluationProgress(
                        snapshot,
                        crate::kernel::worker::available_memory()?,
                    ),
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
                    stream,
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
            protocol::Message::RequestCoupon { task, result } => {
                let encoded =
                    postcard::to_allocvec(&(&task, result)).map_err(|_| Error::WrongType)?;
                let (status, coupon) = Io
                    .request(tree![Blob::create(b"remote_coupon"), Blob::create(encoded),])
                    .await?;
                if status == 0 {
                    send_coupon(stream, &task, &coupon).await?;
                } else {
                    protocol::asynchronous::send(stream, protocol::Message::Rejected).await?;
                }
                continue;
            }
            protocol::Message::RequestTransfer(name) => {
                if name[31] & 0x1f == 0
                    && let Ok(value) = crate::storage::reference_named(name)
                {
                    let loaded = policy.load(&value).await;
                    if let Ok(value) = loaded {
                        crate::storage::put(&value)?;
                    }
                }
                let proposal = match super::propose_object(name, true) {
                    Ok(proposal) => proposal,
                    Err(error) => {
                        protocol::asynchronous::send(stream, protocol::Message::Failure(error))
                            .await?;
                        continue;
                    }
                };
                protocol::asynchronous::send_proposal(
                    stream,
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
                    stream,
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
                protocol::asynchronous::send(stream, reply).await?;
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
                protocol::asynchronous::send(stream, reply).await?;
                continue;
            }
            protocol::Message::ProposeTransfer { task: None, .. }
            | protocol::Message::UpdateLabel { .. } => {
                protocol::asynchronous::send(stream, protocol::Message::Rejected).await?;
                continue;
            }
            _ => (),
        }
        let information = capacity(memory)?;
        let Some(incoming) = protocol::asynchronous::receive_transfer_with_info_from(
            stream,
            protocol::MAX_MESSAGE_SIZE,
            &information,
            keys,
            |offers| missing(offers).ok().flatten(),
            Some(first),
        )
        .await?
        else {
            return Ok(None);
        };
        if incoming.result.is_some() {
            return Err(Error::WrongType.into());
        }
        let task = incoming.task.as_ref().ok_or(protocol::Error::Invalid)?;
        if task.is_stage_programs() && !allow_updates {
            protocol::asynchronous::send(
                stream,
                protocol::Message::Failure(String::from("program staging requires SSH")),
            )
            .await?;
            continue;
        }
        let encoded = incoming.transfer.encode().map_err(|_| Error::WrongType)?;
        let roots = match import(&encoded) {
            Ok(roots) => roots,
            Err(error) => {
                protocol::asynchronous::send(stream, protocol::Message::Failure(error)).await?;
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
                protocol::asynchronous::send(stream, protocol::Message::Failure(error)).await?;
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
                stream,
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
        let reporting = crate::progress::operation(task.operation, input);
        let evaluation = async {
            let result = if let Some(graph) = graph {
                graph.defer_detailed(&node).await.map_err(|failure| {
                    failure
                        .detail
                        .map_or(ConnectionError::Shell(failure.code), |detail| {
                            ConnectionError::Operation(detail.to_string())
                        })
                })?
            } else {
                node.dispatch(policy).await?
            };
            collector::validate(result, task.operation, input)?
                .ok_or(ConnectionError::Shell(Error::WrongType))
        };
        let result = if matches!(&stream, Connection::Request(_)) {
            reporting.finish(evaluation.await)?
        } else {
            let observer = super::ProgressObserver::new()?;
            let mut evaluation = core::pin::pin!(evaluation);
            loop {
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
                if let Some(result) = result {
                    let result = reporting.finish(result);
                    protocol::asynchronous::send(
                        stream,
                        protocol::Message::EvaluationProgress(
                            observer.snapshot()?,
                            crate::kernel::worker::available_memory()?,
                        ),
                    )
                    .await?;
                    break result?;
                }
                protocol::asynchronous::send(
                    stream,
                    protocol::Message::EvaluationProgress(
                        observer.snapshot()?,
                        crate::kernel::worker::available_memory()?,
                    ),
                )
                .await?;
            }
        };
        send_coupon(stream, task, &result).await?;
    }
}

async fn send_coupon(
    stream: &mut Connection,
    task: &protocol::Task,
    result: &Value,
) -> Result<(), ConnectionError> {
    let result = crate::storage::put(result)?;
    let name = result.raw_handle_bytes()?;
    let claim = collector::claim(&result)?.into_value();
    let mut proposal = super::propose(&[result, claim]).map_err(ConnectionError::Operation)?;
    proposal.transfer.roots = alloc::vec![name];
    protocol::asynchronous::send_proposal(
        stream,
        protocol::MAX_MESSAGE_SIZE,
        Some(task),
        Some(name),
        proposal,
        super::read_object,
        |name, keys| certify(name, keys).ok(),
    )
    .await?;
    Ok(())
}

pub(crate) fn work<'a>(
    policy: &'a (impl Scheduler + ?Sized),
    graph: Option<&'a Graph>,
    tasks: &mut crate::executor::task::Tasks<'a, Result<(), Error>>,
) -> Result<bool, Error> {
    for stream in super::network::Network::get()?.incoming() {
        tasks.spawn(async move {
            let keys = trusted_keys()?;
            match connection(stream, u64::MAX, &keys, false, policy, graph).await {
                Ok(_) | Err(ConnectionError::Protocol(protocol::Error::IncompleteTransfer)) => (),
                Err(error) => log(&alloc::format!("remote request failed: {error}"))?,
            }
            Ok(())
        });
    }
    let (progress, completed) = tasks.poll();
    for result in completed {
        result?;
    }
    Ok(progress || tasks.is_ready())
}

/// Serve connections while polling evaluation tasks.
/// An idle connection suspends its I/O future without occupying an evaluation worker.
pub async fn serve(
    input: &Value,
    policy: &(impl Scheduler + ?Sized),
    graph: Option<&Graph>,
    io: &Io,
    stream: Option<u64>,
) -> Result<Value, Error> {
    let local = input.len()? == 0;
    let memory = if local {
        u64::MAX
    } else {
        child(input, 2)?.read_u64()?
    };
    let keys = trusted_keys()?;
    let stdio = !local && crate::shell::as_tree(input)?.len()? > 3 && as_bool(&child(input, 3)?)?;
    let listener = if local || stdio {
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
    let parent = poll_fn(|cx| Poll::Ready(cx.waker().clone())).await;
    let mut accepting =
        listener.map(|listener| Task::with_parent(accept(io, listener), Some(parent.clone())));
    let mut connections = Vec::new();
    let mut accepting_local =
        local.then(|| Task::with_parent(super::local::next(io), Some(parent.clone())));
    let mut local_requests = crate::executor::task::Tasks::with_parent(parent.clone());
    if stdio {
        let stream = if let Some(stream) = stream {
            stream
        } else {
            let (status, stream) = crate::abi::request(tree![Blob::create(b"stdio_stream")])?;
            if status != 0 {
                return Err(Error::WrongType);
            }
            stream.read_u64()?
        };
        connections.push(Task::with_parent(
            connection(Connection::raw(stream), memory, &keys, true, policy, graph),
            Some(parent.clone()),
        ));
    }
    poll_fn(|cx| {
        let mut progress = false;
        if let Some(accepting) = &mut accepting_local
            && let Poll::Ready(request) = accepting.poll()
        {
            let request = match request {
                Ok(request) => request,
                Err(error) => return Poll::Ready(Err(error)),
            };
            local_requests.spawn(super::local::serve(request, policy, graph, io));
            accepting_local = Some(Task::with_parent(
                super::local::next(io),
                Some(parent.clone()),
            ));
            progress = true;
        }
        let (ran, completed) = local_requests.poll();
        progress |= ran;
        for result in completed {
            if let Err(error) = result {
                return Poll::Ready(Err(error));
            }
        }
        if let (Some(accepting_task), Some(listener)) = (&mut accepting, listener)
            && let Poll::Ready(stream) = accepting_task.poll()
        {
            let stream = match stream {
                Ok(stream) => stream,
                Err(error) => return Poll::Ready(Err(error)),
            };
            connections.push(Task::with_parent(
                connection(stream, memory, &keys, false, policy, graph),
                Some(parent.clone()),
            ));
            accepting = Some(Task::with_parent(
                accept(io, listener),
                Some(parent.clone()),
            ));
            progress = true;
        }
        let mut index = 0;
        while index < connections.len() {
            progress |= connections[index].is_ready();
            if let Poll::Ready(result) = connections[index].poll() {
                match result {
                    Ok(Some(programs)) => return Poll::Ready(Ok(programs)),
                    Ok(None) => (),
                    Err(error) => {
                        if let Err(error) = log(&alloc::format!(
                            "remote request failed (protocol {}): {error}",
                            protocol::VERSION
                        )) {
                            return Poll::Ready(Err(error));
                        }
                    }
                }
                drop(connections.swap_remove(index));
                progress = true;
            } else {
                index += 1;
            }
        }
        if stdio && connections.is_empty() {
            return Poll::Ready(crate::shell::capture(tree![]));
        }
        if progress
            || accepting_local.as_ref().is_some_and(Task::is_ready)
            || local_requests.is_ready()
            || accepting.as_ref().is_some_and(Task::is_ready)
            || connections.iter().any(Task::is_ready)
        {
            cx.waker().wake_by_ref();
        }
        Poll::Pending
    })
    .await
}

pub fn run(
    input: &Value,
    policy: &(impl Scheduler + ?Sized),
    graph: Option<&Graph>,
    io: &Io,
) -> Result<Value, Error> {
    crate::executor::drive(serve(input, policy, graph, io, None), policy, graph, io)?
}
