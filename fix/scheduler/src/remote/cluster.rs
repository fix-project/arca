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
use fixwasm::{Error, ThunkType, Value as AsValue, ValueType};

struct Peer {
    name: String,
    connection: Connection,
    information: Information,
}

#[derive(Default)]
struct PeerSlot {
    peer: Option<Peer>,
    polling: bool,
    information: Information,
    waiters: Vec<core::task::Waker>,
}

impl PeerSlot {
    fn new(peer: Option<Peer>) -> Self {
        let information = peer
            .as_ref()
            .map(|peer| peer.information.clone())
            .unwrap_or_default();
        Self {
            peer,
            information,
            ..Self::default()
        }
    }

    async fn take(slot: &Rc<RefCell<Self>>) -> Option<Peer> {
        core::future::poll_fn(|cx| {
            let mut state = slot.borrow_mut();
            if state.polling {
                if !state
                    .waiters
                    .iter()
                    .any(|waker| waker.will_wake(cx.waker()))
                {
                    state.waiters.push(cx.waker().clone());
                }
                core::task::Poll::Pending
            } else {
                core::task::Poll::Ready(state.peer.take())
            }
        })
        .await
    }
}

struct StatusPoll {
    slot: Rc<RefCell<PeerSlot>>,
    peer: Option<Peer>,
}

impl Drop for StatusPoll {
    fn drop(&mut self) {
        let mut slot = self.slot.borrow_mut();
        slot.polling = false;
        for waiter in slot.waiters.drain(..) {
            waiter.wake();
        }
    }
}

async fn status(peer: &mut Peer) -> Result<(), String> {
    protocol::asynchronous::send(&mut peer.connection, Message::RequestProgress)
        .await
        .map_err(|error| alloc::format!("status request failed: {error}"))?;
    match protocol::asynchronous::receive(&mut peer.connection, protocol::MAX_MESSAGE_SIZE).await {
        Ok(Some(Message::EvaluationProgress(snapshot))) => {
            super::report_progress(&peer.name, &snapshot, false)
                .map_err(|error| alloc::format!("invalid status: {error:?}"))?;
            Ok(())
        }
        response => Err(alloc::format!("status response failed: {response:?}")),
    }
}

async fn monitor(slot: Weak<RefCell<PeerSlot>>) {
    loop {
        if Io
            .request(fixwasm::tree![fixwasm::Blob::create(b"sleep"), 1000u64])
            .await
            .is_err()
        {
            return;
        }
        let Some(slot) = slot.upgrade() else { return };
        let peer = {
            let mut state = slot.borrow_mut();
            let Some(peer) = state.peer.take() else {
                continue;
            };
            state.information = peer.information.clone();
            state.polling = true;
            peer
        };
        let mut polling = StatusPoll {
            slot,
            peer: Some(peer),
        };
        let peer = polling.peer.as_mut().unwrap();
        match status(peer).await {
            Ok(()) => {
                polling.slot.borrow_mut().peer = polling.peer.take();
            }
            Err(error) => {
                super::report_status(&peer.name, "unavailable", None, &error);
                return;
            }
        }
    }
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

pub struct Cluster {
    peers: Vec<Rc<RefCell<PeerSlot>>>,
    _monitors: Vec<crate::io::Background>,
    failures: Vec<Option<String>>,
    inventories: Vec<Information>,
}

/// One outstanding transfer owns its connection until completion.
pub struct PendingTransfer {
    slot: Weak<RefCell<PeerSlot>>,
    peer: Option<Peer>,
    task: Option<Task>,
    input: Value,
    names: Vec<super::protocol::Name>,
}

impl Drop for PendingTransfer {
    fn drop(&mut self) {
        if let Some(peer) = self.peer.take() {
            drop(peer.connection);
        }
    }
}

impl Cluster {
    pub async fn connect(io: &Io, inventory: bool) -> Result<Self, Error> {
        let mut peers = Vec::new();
        let mut monitors = Vec::new();
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
                peers.push(Rc::new(RefCell::new(PeerSlot::default())));
                failures.push(failure);
                continue;
            }
            super::report_status(&remote.name, "connecting", None, "");
            inventories.push(Information::default());
            let mut failure = None;
            let mut peer = (async {
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
                    match Connection::connect(io, &remote).await {
                        Ok(Some(connection)) => connection,
                        Ok(None) => {
                            failure = Some(String::from("connection closed before handshake"));
                            return None;
                        }
                        Err(error) => {
                            failure = Some(alloc::format!("connection failed: {error:?}"));
                            return None;
                        }
                    }
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
            match &peer {
                Some(peer) => {
                    super::report_status(&remote.name, "available", Some(&peer.information), "")
                }
                None => super::report_status(
                    &remote.name,
                    "unavailable",
                    None,
                    failure.as_deref().unwrap_or("connection did not complete"),
                ),
            }
            if let Some(peer) = &mut peer
                && let Err(error) = status(peer).await
            {
                super::report_status(&remote.name, "unavailable", None, &error);
                failure = Some(error);
            }
            if failure.is_some() {
                peer = None;
            }
            let slot = Rc::new(RefCell::new(PeerSlot::new(peer)));
            if slot.borrow().peer.is_some() {
                monitors.push(io.spawn(monitor(Rc::downgrade(&slot))));
            }
            peers.push(slot);
            failures.push(failure);
        }
        Ok(Self {
            peers,
            _monitors: monitors,
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
                let slot = peer.borrow();
                if slot.polling {
                    slot.information.clone()
                } else {
                    slot.peer
                        .as_ref()
                        .map_or_else(|| inventory.clone(), |peer| peer.information.clone())
                }
            })
            .collect()
    }

    /// Request data using the same proposal, acceptance, and import exchange as evaluations.
    pub async fn request_transfer(
        &self,
        location: usize,
        input: &Value,
    ) -> Result<Option<Value>, Failure> {
        let Some(slot) = location
            .checked_sub(1)
            .and_then(|index| self.peers.get(index))
        else {
            return Ok(None);
        };
        let Some(peer) = PeerSlot::take(slot).await else {
            return Ok(None);
        };
        let mut name = storage::canonical_value(input)?.raw_handle_bytes()?;
        name[31] &= 0xe0;
        let mut pending = PendingTransfer {
            slot: Rc::downgrade(slot),
            peer: Some(peer),
            task: None,
            input: input.clone(),
            names: Vec::new(),
        };
        protocol::asynchronous::send(
            &mut pending.peer.as_mut().unwrap().connection,
            Message::RequestTransfer(name),
        )
        .await
        .map_err(Failure::Send)?;
        pending.receive().await.map(Some)
    }

    /// Transfer the inputs and start evaluation, leaving completion to `receive`.
    pub async fn submit(
        &self,
        location: usize,
        operation: u64,
        input: &Value,
    ) -> Result<Option<PendingTransfer>, Failure> {
        if !matches!(operation, 1 | 4 | 2 | 3 | 6) || location == 0 {
            return Ok(None);
        }
        let Some(slot) = self.peers.get(location - 1) else {
            return Ok(None);
        };
        if slot.borrow().peer.is_none() && !slot.borrow().polling {
            return Ok(None);
        }
        let collector = storage::put(&crate::collector::callable()?)?;
        let input = storage::put(input)?;
        let descriptor_only = (operation == 1 && input.is_value()?)
            || (matches!(input.value_type(), ValueType::Thunk | ValueType::Encode)
                && input.unsound_thunk_type()? == ThunkType::Identification
                && (operation != 1 || input.is_shallow()?));
        let roots = [collector, input.clone()];
        let proposal = if descriptor_only {
            super::propose(&roots)
        } else {
            super::propose_full(&roots)
        }
        .map_err(Failure::Export)?;
        self.submit_transfer(location, operation, &input, proposal)
            .await
    }

    /// Submit a proposal with caller-selected backing data and TODOs.
    pub async fn submit_transfer(
        &self,
        location: usize,
        operation: u64,
        input: &Value,
        proposal: (Transfer, Vec<protocol::Name>),
    ) -> Result<Option<PendingTransfer>, Failure> {
        let (transfer, backing) = proposal;
        let Some(slot) = location
            .checked_sub(1)
            .and_then(|index| self.peers.get(index))
        else {
            return Ok(None);
        };
        if slot.borrow().peer.is_none() && !slot.borrow().polling {
            return Ok(None);
        }
        let collector = storage::put(&crate::collector::callable()?)?;
        let input = storage::put(input)?;
        let names = transfer
            .records
            .iter()
            .filter(|record| record.data.is_some() || backing.contains(&record.name))
            .map(|record| record.name)
            .collect();
        let task = Task {
            operation,
            collector: collector.raw_handle_bytes()?,
            input: input.raw_handle_bytes()?,
        };
        let Some(peer) = PeerSlot::take(slot).await else {
            return Ok(None);
        };
        let mut pending = PendingTransfer {
            slot: Rc::downgrade(slot),
            peer: Some(peer),
            task: Some(task),
            input,
            names,
        };
        let remote = pending.peer.as_ref().unwrap().name.clone();
        let mut input_name = pending.input.raw_handle_bytes()?;
        input_name[31] &= 0xe0;
        let source = if backing.contains(&input_name) {
            pending.input.clone()
        } else {
            collector.clone()
        };
        let mut load = None;
        let sent = protocol::asynchronous::send_proposal(
            &mut pending.peer.as_mut().unwrap().connection,
            protocol::MAX_MESSAGE_SIZE,
            pending.task.as_ref(),
            None,
            (transfer, backing),
            |name| {
                load.get_or_insert_with(|| crate::progress::load(&source, "", &remote));
                super::read_object(name)
            },
            |name, keys| super::certify(name, keys).ok(),
        )
        .await;
        if let Some(load) = load {
            load.finish(sent.is_ok());
        }
        sent.map_err(Failure::Send)?;
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

async fn stage_programs(io: &Io, remote: &super::Remote) -> Result<Connection, String> {
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
    let proposal = super::propose(&[collector.clone(), scheduler.clone()])?;
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
    let mut scheduler_name = scheduler
        .raw_handle_bytes()
        .map_err(|error| alloc::format!("{error:?}"))?;
    scheduler_name[31] &= 0xe0;
    let source = if proposal.1.contains(&scheduler_name) {
        &scheduler
    } else {
        &collector
    };
    let mut load = None;
    let result = async {
        protocol::asynchronous::send_proposal(
            &mut initial,
            protocol::MAX_MESSAGE_SIZE,
            Some(&task),
            None,
            proposal,
            |name| {
                load.get_or_insert_with(|| crate::progress::load(source, "", &remote.name));
                super::read_object(name)
            },
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
    .await;
    if let Some(load) = load {
        load.finish(result.is_ok());
    }
    result
}

impl PendingTransfer {
    /// Import an accepted transfer, checking the expected result or evaluation coupon.
    pub async fn receive(mut self) -> Result<Value, Failure> {
        let trusted_keys = super::trusted_keys()?;
        let remote = self.peer.as_ref().ok_or(Failure::Closed)?.name.clone();
        let mut download = None;
        let result = async {
            let peer = self.peer.as_mut().ok_or(Failure::Closed)?;
            let incoming = protocol::asynchronous::receive_transfer_observed(
                &mut peer.connection,
                protocol::MAX_MESSAGE_SIZE,
                &Information {
                    parallelism: worker::count()?.try_into().map_err(|_| Failure::Task)?,
                    ..Information::default()
                },
                &trusted_keys,
                |offers| {
                    let missing = super::missing(offers).ok().flatten()?;
                    let data = offers
                        .iter()
                        .filter(|offer| offer.backing && missing.contains(&offer.name))
                        .max_by_key(|offer| {
                            (
                                crate::progress::named(offer.name).unwrap_or(false),
                                fixhandle::Handle::parse_ref(&offer.name)
                                    .map(|value| value.byte_len())
                                    .unwrap_or(0),
                            )
                        });
                    if let Some(data) = data
                        && let Ok(value) = storage::reference_named(data.name)
                    {
                        download = Some(crate::progress::load(&value, &remote, ""));
                    }
                    Some(missing)
                },
                None,
                |message| {
                    if let Message::EvaluationProgress(snapshot) = message {
                        let _ = super::report_progress(&remote, snapshot, true);
                    }
                },
            )
            .await
            .map_err(Failure::Receive)?
            .ok_or(Failure::Closed)?;
            if incoming.task != self.task || incoming.result.is_none() {
                return Err(Failure::Task);
            }
            let reusable = match status(peer).await {
                Ok(()) => true,
                Err(error) => {
                    super::report_status(&remote, "unavailable", None, &error);
                    false
                }
            };
            let bytes = incoming.transfer.encode().map_err(|_| Failure::Task)?;
            let roots = super::import(&bytes).map_err(Failure::Import)?;
            let [coupon] = roots.as_slice() else {
                return Err(Failure::Coupon);
            };
            let coupon = if let Some(task) = &self.task {
                crate::collector::validate(coupon.clone(), task.operation, &self.input)?
                    .ok_or(Failure::Coupon)?
            } else {
                let mut expected = storage::canonical_value(&self.input)?.raw_handle_bytes()?;
                expected[31] &= 0xe0;
                if incoming.result != Some(expected) || coupon.raw_handle_bytes()? != expected {
                    return Err(Failure::Task);
                }
                coupon.clone()
            };
            peer.information.data.append(&mut self.names);
            if let Some(task) = &self.task
                && !peer.information.results.contains(task)
            {
                peer.information.results.push(task.clone());
            }
            if reusable && let Some(slot) = self.slot.upgrade() {
                slot.borrow_mut().peer = self.peer.take();
            }

            Ok(coupon)
        }
        .await;
        if let Some(mut load) = download {
            if let Ok(value) = &result {
                load.value(value);
            }
            load.finish(result.is_ok());
        }
        result
    }
}

impl Drop for Cluster {
    fn drop(&mut self) {
        for peer in &mut self.peers {
            if let Some(peer) = peer.borrow_mut().peer.take() {
                drop(peer.connection);
            }
        }
    }
}
