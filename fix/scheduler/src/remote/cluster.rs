use super::{
    Connection, configured,
    protocol::{self, Information, Message, Task, Transfer},
};
use crate::{
    io::{Io, log},
    kernel::worker,
    shell::{Value, capture, child},
    storage,
};
use alloc::{
    rc::{Rc, Weak},
    string::String,
    vec::Vec,
};
use core::cell::RefCell;
use fixwasm::{Error, Value as AsValue};

struct Peer<'a> {
    name: String,
    connection: Connection<'a>,
    information: Information,
}

#[derive(Debug)]
pub enum Failure {
    Shell(Error),
    Export(String),
    Send(protocol::Error<super::IoError>),
    Receive(protocol::Error<super::IoError>),
    Closed,
    Task,
    Import(String),
    Coupon,
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self::Shell(error)
    }
}

pub struct Cluster<'a> {
    peers: Vec<Rc<RefCell<Option<Peer<'a>>>>>,
    failures: Vec<Option<String>>,
    inventories: Vec<Information>,
}

/// One outstanding evaluation owns its connection until completion.
pub struct PendingEvaluation<'a> {
    slot: Weak<RefCell<Option<Peer<'a>>>>,
    peer: Option<Peer<'a>>,
    task: Task,
    input: Value,
    names: Vec<super::protocol::Name>,
    operation: u64,
}

impl Drop for PendingEvaluation<'_> {
    fn drop(&mut self) {
        if let Some(peer) = self.peer.take() {
            drop(peer.connection);
        }
    }
}

impl<'a> Cluster<'a> {
    pub async fn connect(io: &'a Io, inventory: bool) -> Result<Self, Error> {
        let mut peers = Vec::new();
        let mut failures = Vec::new();
        let mut inventories = Vec::new();
        for remote in configured()? {
            if remote.address.starts_with("file://") {
                let information = if inventory {
                    super::file_information(&remote.address)
                } else {
                    Ok(Information::default())
                };
                let (information, failure) = match information {
                    Ok(information) => (information, None),
                    Err(error) => {
                        log(&alloc::format!("remote {}: {error}", remote.name))?;
                        (Information::default(), Some(error))
                    }
                };
                inventories.push(information);
                peers.push(Rc::new(RefCell::new(None)));
                failures.push(failure);
                continue;
            }
            inventories.push(Information::default());
            let mut failure = None;
            let peer = (async {
                let mut connection = if remote.is_ssh()
                    && (remote.scheduler.is_none() || remote.collector.is_none())
                {
                    match stage_programs(io, &remote).await {
                        Ok(connection) => connection,
                        Err(error) => {
                            let _ = log(&alloc::format!("remote {}: {error}", remote.name));
                            failure = Some(error);
                            return None;
                        }
                    }
                } else {
                    Connection::connect(io, &remote).await.ok().flatten()?
                };
                if let Err(error) = protocol::asynchronous::send(
                    &mut connection,
                    Message::RequestInfo { inventory },
                )
                .await
                {
                    failure = Some(alloc::format!("capacity request failed: {error}"));
                    drop(connection);
                    return None;
                }
                match protocol::asynchronous::receive(&mut connection, protocol::MAX_MESSAGE_SIZE)
                    .await
                {
                    Ok(Some(Message::Info(information))) => Some(Peer {
                        name: remote.name.clone(),
                        connection,
                        information,
                    }),
                    response => {
                        failure = Some(match response {
                            Ok(Some(Message::Failure(message))) => message,
                            Ok(message) => {
                                alloc::format!("unexpected capacity response: {message:?}")
                            }
                            Err(error) => alloc::format!("capacity response failed: {error}"),
                        });
                        drop(connection);
                        None
                    }
                }
            })
            .await;
            peers.push(Rc::new(RefCell::new(peer)));
            failures.push(failure);
        }
        Ok(Self {
            peers,
            failures,
            inventories,
        })
    }

    pub fn first_failure(&self) -> Option<&str> {
        self.failures.iter().find_map(|failure| failure.as_deref())
    }

    pub fn information(&self) -> Vec<Information> {
        self.peers
            .iter()
            .zip(&self.inventories)
            .map(|(peer, inventory)| {
                peer.borrow()
                    .as_ref()
                    .map_or_else(|| inventory.clone(), |peer| peer.information.clone())
            })
            .collect()
    }

    /// Transfer the inputs and start evaluation, leaving completion to `receive`.
    pub async fn submit(
        &self,
        location: usize,
        operation: u64,
        input: &Value,
    ) -> Result<Option<PendingEvaluation<'a>>, Failure> {
        if !matches!(operation, 1 | 4 | 2 | 3 | 6) || location == 0 {
            return Ok(None);
        }
        let Some(slot) = self.peers.get(location - 1) else {
            return Ok(None);
        };
        if slot.borrow().is_none() {
            return Ok(None);
        }
        let collector = storage::put(&crate::collector::callable()?)?;
        let input = storage::put(input)?;
        let bytes =
            super::export(&[collector.clone(), input.clone()], &[]).map_err(Failure::Export)?;
        let transfer = Transfer::decode(&bytes).map_err(|_| Error::WrongType)?;
        self.submit_transfer(location, operation, &input, transfer)
            .await
    }

    /// Submit a caller-prepared transfer, including its choice of backing data and TODOs.
    pub async fn submit_transfer(
        &self,
        location: usize,
        operation: u64,
        input: &Value,
        transfer: Transfer,
    ) -> Result<Option<PendingEvaluation<'a>>, Failure> {
        let Some(slot) = location
            .checked_sub(1)
            .and_then(|index| self.peers.get(index))
        else {
            return Ok(None);
        };
        if slot.borrow().is_none() {
            return Ok(None);
        }
        let collector = storage::put(&crate::collector::callable()?)?;
        let input = storage::put(input)?;
        let names = transfer
            .records
            .iter()
            .filter(|record| record.data.is_some())
            .map(|record| record.name)
            .collect();
        let task = Task {
            operation,
            collector: collector.raw_handle_bytes()?,
            input: input.raw_handle_bytes()?,
        };
        let mut pending = PendingEvaluation {
            slot: Rc::downgrade(slot),
            peer: slot.borrow_mut().take(),
            task,
            input,
            names,
            operation,
        };
        let remote = pending.peer.as_ref().unwrap().name.clone();
        let _load = crate::progress::load(&pending.input, "", &remote);
        protocol::asynchronous::send_transfer(
            &mut pending.peer.as_mut().unwrap().connection,
            protocol::MAX_MESSAGE_SIZE,
            &pending.task,
            None,
            transfer,
            |name, keys| super::certify(name, keys).ok(),
        )
        .await
        .map_err(Failure::Send)?;
        Ok(Some(pending))
    }

    pub async fn execute(
        &self,
        location: usize,
        operation: u64,
        input: &Value,
    ) -> Result<Option<Value>, Failure> {
        match self.submit(location, operation, input).await? {
            Some(pending) => pending.receive().await.map(Some),
            None => Ok(None),
        }
    }
}

async fn stage_programs<'a>(io: &'a Io, remote: &super::Remote) -> Result<Connection<'a>, String> {
    let programs =
        super::effect(b"remote_programs", &[]).map_err(|error| alloc::format!("{error:?}"))?;
    let empty = capture(fixwasm::Blob::create(b"")).map_err(|error| alloc::format!("{error:?}"))?;
    let scheduler = if remote.scheduler.is_some() {
        empty.clone()
    } else {
        child(&programs, 0).map_err(|error| alloc::format!("{error:?}"))?
    };
    let collector = if remote.collector.is_some() {
        empty
    } else {
        child(&programs, 1).map_err(|error| alloc::format!("{error:?}"))?
    };
    let scheduler = storage::put(&scheduler).map_err(|error| alloc::format!("{error:?}"))?;
    let collector = storage::put(&collector).map_err(|error| alloc::format!("{error:?}"))?;
    let transfer = super::export(&[collector.clone(), scheduler.clone()], &[])?;
    let transfer = Transfer::decode(&transfer)
        .map_err(|error| alloc::format!("invalid program transfer: {error}"))?;
    let task = Task::stage_programs(
        collector
            .raw_handle_bytes()
            .map_err(|error| alloc::format!("{error:?}"))?,
        scheduler
            .raw_handle_bytes()
            .map_err(|error| alloc::format!("{error:?}"))?,
    );
    let mut initial = Connection::connect(io, remote)
        .await
        .map_err(|error| alloc::format!("{error:?}"))?
        .ok_or_else(|| String::from("could not start bootstrap server"))?;
    let _load = crate::progress::load(&scheduler, "", &remote.name);
    protocol::asynchronous::send_transfer(
        &mut initial,
        protocol::MAX_MESSAGE_SIZE,
        &task,
        None,
        transfer,
        |name, keys| super::certify(name, keys).ok(),
    )
    .await
    .map_err(|error| alloc::format!("program transfer failed: {error}"))?;
    match protocol::asynchronous::receive(&mut initial, protocol::MAX_MESSAGE_SIZE)
        .await
        .map_err(|error| alloc::format!("program transfer failed: {error}"))?
    {
        Some(Message::ObjectReceived(name)) if name == task.input => (),
        Some(Message::Failure(error)) => return Err(error),
        response => {
            return Err(alloc::format!(
                "unexpected program transfer response: {response:?}"
            ));
        }
    }
    Ok(initial)
}

impl PendingEvaluation<'_> {
    /// Wait for a submitted evaluation and return its validated coupon.
    pub async fn receive(mut self) -> Result<Value, Failure> {
        let trusted_keys = super::trusted_keys()?;
        let peer = self.peer.as_mut().ok_or(Failure::Closed)?;
        let remote = peer.name.clone();
        let mut download = None;
        let incoming = protocol::asynchronous::receive_transfer_observed(
            &mut peer.connection,
            protocol::MAX_MESSAGE_SIZE,
            &Information {
                parallelism: worker::count()?.try_into().map_err(|_| Failure::Task)?,
                ..Information::default()
            },
            &trusted_keys,
            |offers| super::missing(offers).ok().flatten(),
            None,
            |message| match message {
                Message::EvaluationProgress(snapshot) => {
                    let _ = super::report_progress(&remote, snapshot);
                }
                Message::ProposeTransfer { .. } => {
                    download.get_or_insert_with(|| crate::progress::load(&self.input, &remote, ""));
                }
                _ => (),
            },
        )
        .await
        .map_err(Failure::Receive)?
        .ok_or(Failure::Closed)?;
        drop(download);
        if incoming.task != self.task || incoming.result.is_none() {
            return Err(Failure::Task);
        }
        let bytes = incoming.transfer.encode().map_err(|_| Failure::Task)?;
        let roots = super::import(&bytes).map_err(Failure::Import)?;
        let [coupon] = roots.as_slice() else {
            return Err(Failure::Coupon);
        };
        let coupon = crate::collector::validate(coupon.clone(), self.operation, &self.input)?
            .ok_or(Failure::Coupon)?;
        peer.information.data.append(&mut self.names);
        if !peer.information.results.contains(&self.task) {
            peer.information.results.push(self.task.clone());
        }
        if let Some(slot) = self.slot.upgrade() {
            *slot.borrow_mut() = self.peer.take();
        }

        Ok(coupon)
    }
}

impl Drop for Cluster<'_> {
    fn drop(&mut self) {
        for peer in &mut self.peers {
            if let Some(peer) = peer.borrow_mut().take() {
                drop(peer.connection);
            }
        }
    }
}
