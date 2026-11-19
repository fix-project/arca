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
use alloc::{rc::Rc, string::String, vec::Vec};
use core::cell::RefCell;
use fixwasm::{Error, ThunkType, Value as AsValue, ValueType};

struct Peer {
    name: String,
    connection: Connection,
    node: protocol::topology::NodeId,
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
    network: Rc<super::network::Network>,
    routes: RefCell<Vec<super::network::Route>>,
    failures: Vec<Option<String>>,
    inventories: Vec<Information>,
}

pub struct PendingTransfer {
    network: Rc<super::network::Network>,
    peer: Option<Peer>,
    task: Option<Task>,
    input: Value,
    names: Vec<super::protocol::Name>,
}

impl Cluster {
    pub async fn connect(io: &Io, inventory: bool) -> Result<Self, Error> {
        let network = super::network::Network::get()?;
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
                failures.push(failure);
                continue;
            }
            super::report_status(&remote.name, "connecting", None, "");
            let mut failure = None;
            let connected = (async {
                let connection = if remote.is_ssh()
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
                match network.connect(connection, remote.name.clone()).await {
                    Ok(()) => Some(()),
                    Err(error) => {
                        failure = Some(alloc::format!("topology handshake failed: {error}"));
                        None
                    }
                }
            })
            .await;
            if connected.is_none() {
                super::report_status(
                    &remote.name,
                    "unavailable",
                    None,
                    failure.as_deref().unwrap_or("connection failed"),
                );
            }
            failures.push(failure);
        }
        Ok(Self {
            network,
            routes: RefCell::new(Vec::new()),
            failures,
            inventories,
        })
    }

    pub fn first_failure(&self) -> Option<&str> {
        self.failures.iter().find_map(|failure| failure.as_deref())
    }

    pub fn information(&self) -> Vec<Information> {
        let discovered = self.network.routes();
        let mut routes = self.routes.borrow_mut();
        for previous in routes.iter_mut() {
            previous.information = Information::default();
        }
        for mut route in discovered {
            route.information.data.sort_unstable();
            route
                .information
                .results
                .sort_unstable_by_key(|task| (task.operation, task.input, task.collector));
            super::report_node_status(
                &route.path.name,
                "available",
                Some(&route.information),
                "",
                Some(route.path.node),
            );
            if let Some(previous) = routes
                .iter_mut()
                .find(|previous| previous.path.node == route.path.node)
            {
                *previous = route;
            } else {
                routes.push(route);
            }
        }
        self.inventories
            .iter()
            .cloned()
            .chain(routes.iter().map(|route| route.information.clone()))
            .collect()
    }

    pub fn sources(&self, value: &Value) -> Result<Vec<(usize, String)>, Error> {
        let mut name = storage::canonical_value(value)?.raw_handle_bytes()?;
        name[31] &= 0xe0;
        self.information();
        Ok(self
            .routes
            .borrow()
            .iter()
            .enumerate()
            .filter(|(_, route)| route.information.data.binary_search(&name).is_ok())
            .map(|(index, route)| (self.inventories.len() + index + 1, route.path.name.clone()))
            .collect())
    }

    fn peer(&self, location: usize) -> Option<Peer> {
        let index = location.checked_sub(self.inventories.len() + 1)?;
        let routes = self.routes.borrow();
        let route = routes.get(index)?;
        let (connection, node, name) = self.network.open(&route.path).ok()?;
        Some(Peer {
            name,
            connection,
            node,
        })
    }

    /// Request data using the same proposal, acceptance, and import exchange as evaluations.
    pub async fn request_transfer(
        &self,
        location: usize,
        input: &Value,
    ) -> Result<Option<Value>, Failure> {
        let Some(peer) = self.peer(location) else {
            return Ok(None);
        };
        let mut name = storage::canonical_value(input)?.raw_handle_bytes()?;
        name[31] &= 0xe0;
        let mut pending = PendingTransfer {
            network: self.network.clone(),
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
        inventory: Option<&Value>,
    ) -> Result<Option<PendingTransfer>, Failure> {
        if !matches!(operation, 1 | 4 | 2 | 3 | 6) || location == 0 {
            return Ok(None);
        }
        let collector = crate::collector::callable()?;
        let collector = persist(&collector).await?;
        let input = persist(input).await?;
        let mut name = input.raw_handle_bytes()?;
        name[31] &= 0xe0;
        let resident = location
            .checked_sub(self.inventories.len() + 1)
            .and_then(|index| {
                self.routes
                    .borrow()
                    .get(index)
                    .map(|route| route.information.data.binary_search(&name).is_ok())
            })
            .unwrap_or(false);
        let descriptor_only = (operation == 1 && input.is_value()?)
            || (matches!(input.value_type(), ValueType::Thunk | ValueType::Encode)
                && input.unsound_thunk_type()? == ThunkType::Identification
                && (operation != 1 || input.is_shallow()?));
        let proposal = super::propose_task(
            &collector,
            &input,
            !resident,
            !resident && !descriptor_only,
            inventory,
        )
        .await
        .map_err(Failure::Export)?;
        self.submit_transfer(location, operation, collector, input, proposal)
            .await
    }

    /// Submit a proposal with caller-selected backing data and TODOs.
    async fn submit_transfer(
        &self,
        location: usize,
        operation: u64,
        collector: Value,
        input: Value,
        proposal: (Transfer, Vec<protocol::Name>),
    ) -> Result<Option<PendingTransfer>, Failure> {
        let (transfer, mut backing) = proposal;
        backing.sort_unstable();
        let names = transfer
            .records
            .iter()
            .filter(|record| record.data.is_some() || backing.binary_search(&record.name).is_ok())
            .map(|record| record.name)
            .collect();
        let task = Task {
            operation,
            collector: collector.raw_handle_bytes()?,
            input: input.raw_handle_bytes()?,
        };
        let Some(peer) = self.peer(location) else {
            return Ok(None);
        };
        if let Connection::Request(request) = &peer.connection {
            request.mark_used();
        }
        let mut pending = PendingTransfer {
            network: self.network.clone(),
            peer: Some(peer),
            task: Some(task),
            input,
            names,
        };
        let remote = pending.peer.as_ref().unwrap().name.clone();
        let mut input_name = pending.input.raw_handle_bytes()?;
        input_name[31] &= 0xe0;
        let source = if backing.binary_search(&input_name).is_ok() {
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
        inventory: Option<&Value>,
    ) -> Result<Option<Value>, Failure> {
        let Some(pending) = self.submit(location, operation, input, inventory).await? else {
            return Ok(None);
        };
        let result = pending.receive().await?;
        if inventory.is_none() {
            self.report_final_progress(location).await;
        }
        Ok(Some(result))
    }

    pub async fn report_final_progress(&self, location: usize) {
        let Some(mut peer) = self.peer(location) else {
            return;
        };
        let report = async {
            protocol::asynchronous::send(&mut peer.connection, Message::RequestProgress).await?;
            if let Some(Message::EvaluationProgress(snapshot, _)) =
                protocol::asynchronous::receive(&mut peer.connection, protocol::MAX_MESSAGE_SIZE)
                    .await?
            {
                let _ = super::report_progress(&peer.name, &snapshot, true);
                peer.connection.finish_request();
            }
            Ok::<(), protocol::Error<super::IoError>>(())
        };
        let timeout = Io.request(fixwasm::tree![fixwasm::Blob::create(b"sleep"), 250u64]);
        let mut report = core::pin::pin!(report);
        let mut timeout = core::pin::pin!(timeout);
        core::future::poll_fn(|cx| {
            if report.as_mut().poll(cx).is_ready() || timeout.as_mut().poll(cx).is_ready() {
                core::task::Poll::Ready(())
            } else {
                core::task::Poll::Pending
            }
        })
        .await;
    }
}

async fn persist(value: &Value) -> Result<Value, Failure> {
    let (status, saved) = Io
        .request(fixwasm::tree![fixwasm::Blob::create(b"storage_put"), value])
        .await?;
    if status != 0 {
        return Err(Failure::Export(super::request_failure(status, &saved)));
    }
    Ok(saved)
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
                    let requested: alloc::collections::BTreeSet<_> = missing.iter().collect();
                    let data = offers
                        .iter()
                        .filter(|offer| offer.backing && requested.contains(&offer.name))
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
                    if let Message::EvaluationProgress(snapshot, _) = message {
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
            self.network
                .completed(peer.node, &self.names, self.task.as_ref());

            Ok(coupon)
        }
        .await;
        if result.is_ok()
            && let Some(peer) = &mut self.peer
        {
            peer.connection.finish_request();
        }
        if let Some(mut load) = download {
            if let Ok(value) = &result {
                load.value(value);
            }
            load.finish(result.is_ok());
        }
        result
    }
}
