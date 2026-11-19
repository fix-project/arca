use super::{
    Connection, configured,
    protocol::{self, Information, Message, Task},
};
use crate::{
    io::{Io, log},
    kernel::worker,
    shell::{Value, capture, child},
    storage,
};
use alloc::{collections::BTreeMap, rc::Rc, string::String, vec::Vec};
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
    AwaitingResult,
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
    collector_name: protocol::Name,
}

pub struct PendingTransfer {
    network: Rc<super::network::Network>,
    peer: Option<Peer>,
    expected: Expected,
    names: Vec<super::protocol::Name>,
}

enum Expected {
    Value(protocol::Name),
    Coupon {
        task: Task,
        input: Value,
        result: Option<protocol::Name>,
    },
}

impl Cluster {
    pub async fn connect(io: &Io, inventory: bool) -> Result<Self, Error> {
        let network = super::network::Network::get()?;
        let peer_failures = network.discover(io).await?;
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
            } else if let Some(failure) = peer_failures.get(&remote.name) {
                failures.push(Some(failure.clone()));
            }
        }
        Ok(Self {
            network,
            routes: RefCell::new(Vec::new()),
            failures,
            inventories,
            collector_name: storage::canonical_value(&crate::collector::callable()?)?
                .raw_handle_bytes()?,
        })
    }

    pub fn first_failure(&self) -> Option<&str> {
        self.failures.iter().find_map(|failure| failure.as_deref())
    }

    pub fn information(&self) -> Vec<Information> {
        let discovered = self.network.routes(Some(self.collector_name));
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

    pub fn workers_with_memory(&self, required: u64) -> Vec<usize> {
        let mut routes = self.routes.borrow_mut();
        let indices: BTreeMap<_, _> = routes
            .iter()
            .enumerate()
            .map(|(index, route)| (route.path.node, index))
            .collect();
        let mut workers: Vec<_> = self
            .network
            .available_routes(required)
            .into_iter()
            .map(|(path, slots)| {
                let index = indices.get(&path.node).copied().unwrap_or_else(|| {
                    let index = routes.len();
                    routes.push(super::network::Route {
                        path,
                        information: Information::default(),
                    });
                    index
                });
                (slots, index + self.inventories.len() + 1)
            })
            .collect();
        workers.sort_by_key(|&(slots, index)| (core::cmp::Reverse(slots), index));
        workers.into_iter().map(|(_, index)| index).collect()
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

    pub fn location(&self, location: usize) -> Option<String> {
        let index = location.checked_sub(self.inventories.len() + 1)?;
        Some(self.routes.borrow().get(index)?.path.name.clone())
    }

    fn peer(&self, location: usize) -> Option<Peer> {
        let index = location.checked_sub(self.inventories.len() + 1)?;
        let routes = self.routes.borrow();
        let route = routes.get(index)?;
        let (connection, node, name) = self.network.open(route.path.node).ok()?;
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
        let mut name = storage::canonical_value(input)?.raw_handle_bytes()?;
        name[31] &= 0xe0;
        self.request_value(location, name).await
    }

    /// Request a value by its complete canonical name.
    pub async fn request_value(
        &self,
        location: usize,
        name: protocol::Name,
    ) -> Result<Option<Value>, Failure> {
        if !fixhandle::Handle::parse_ref(&name).is_ok_and(|handle| handle.is_canonical()) {
            return Err(Failure::Task);
        }
        let Some(peer) = self.peer(location) else {
            return Ok(None);
        };
        let mut pending = PendingTransfer {
            network: self.network.clone(),
            peer: Some(peer),
            expected: Expected::Value(name),
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
        proposal: protocol::Proposal,
    ) -> Result<Option<PendingTransfer>, Failure> {
        let mut proposal = proposal;
        proposal.backing.sort_unstable();
        let names = proposal
            .transfer
            .records
            .iter()
            .filter(|record| {
                record.data.is_some() || proposal.backing.binary_search(&record.name).is_ok()
            })
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
        if self.network.awaiting_result(peer.node, &task) {
            return Err(Failure::AwaitingResult);
        }
        if let Connection::Request(request) = &peer.connection {
            request.mark_used();
        }
        let mut peer = peer;
        let mut input_name = task.input;
        input_name[31] &= 0xe0;
        let source = if proposal.backing.binary_search(&input_name).is_ok() {
            &input
        } else {
            &collector
        };
        let mut load = None;
        let sent = protocol::asynchronous::send_proposal(
            &mut peer.connection,
            protocol::MAX_MESSAGE_SIZE,
            Some(&task),
            None,
            proposal,
            |name| {
                load.get_or_insert_with(|| {
                    crate::progress::load(source, "", &peer.name, Some(peer.node))
                });
                super::read_object(name)
            },
            |name, keys| super::certify(name, keys).ok(),
        )
        .await;
        if let Some(load) = load {
            load.finish(sent.is_ok(), sent.as_ref().ok().copied());
        }
        sent.map_err(Failure::Send)?;
        Ok(Some(PendingTransfer {
            network: self.network.clone(),
            peer: Some(peer),
            expected: Expected::Coupon {
                task,
                input,
                result: None,
            },
            names,
        }))
    }

    pub async fn execute(
        &self,
        location: usize,
        operation: u64,
        input: &Value,
        inventory: Option<&Value>,
    ) -> Result<Option<Value>, Failure> {
        if protocol::CouponOffer::supports(operation)
            && let Some(result) = self.fetch_coupon(location, operation, input).await
        {
            return Ok(Some(result));
        }
        let Some(pending) = self.submit(location, operation, input, inventory).await? else {
            return Ok(None);
        };
        let result = pending.receive().await?;
        if inventory.is_none() {
            self.report_final_progress(location).await;
        }
        Ok(Some(result))
    }

    async fn fetch_coupon(&self, location: usize, operation: u64, input: &Value) -> Option<Value> {
        let node = self
            .routes
            .borrow()
            .get(location.checked_sub(self.inventories.len() + 1)?)?
            .path
            .node;
        if !self.network.offers_coupons(node) {
            return None;
        }
        let mut name = input.raw_handle_bytes().ok()?;
        if !fixhandle::Handle::parse_ref(&name).ok()?.is_canonical() {
            name = storage::canonical_value(input)
                .ok()?
                .raw_handle_bytes()
                .ok()?;
        }
        let task = Task {
            operation,
            collector: self.collector_name,
            input: name,
        };
        let result = self.network.coupon(node, &task)?;
        let mut peer = self.peer(location)?;
        protocol::asynchronous::send(
            &mut peer.connection,
            Message::RequestCoupon {
                task: task.clone(),
                result,
            },
        )
        .await
        .ok()?;
        PendingTransfer {
            network: self.network.clone(),
            peer: Some(peer),
            expected: Expected::Coupon {
                task,
                input: input.clone(),
                result: Some(result),
            },
            names: Vec::new(),
        }
        .receive()
        .await
        .ok()
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

pub(super) async fn discover_peers(
    network: &Rc<super::network::Network>,
) -> Result<BTreeMap<String, String>, Error> {
    let mut failures = BTreeMap::new();
    for remote in configured()?
        .into_iter()
        .filter(|remote| !remote.address.starts_with("file://"))
    {
        super::report_status(&remote.name, "connecting", None, "");
        let mut failure = None;
        let connected = (async {
            let connection =
                if remote.is_ssh() && (remote.scheduler.is_none() || remote.collector.is_none()) {
                    match stage_programs(&Io, &remote).await {
                        Ok(connection) => connection,
                        Err(error) => {
                            let _ = log(&alloc::format!("remote {}: {error}", remote.name));
                            failure = Some(error);
                            return None;
                        }
                    }
                } else {
                    match Connection::connect(&Io, &remote).await {
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
        if let Some(failure) = failure {
            failures.insert(remote.name, failure);
        }
    }
    Ok(failures)
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
    let source = if proposal.backing.contains(&scheduler_name) {
        &scheduler
    } else {
        &collector
    };
    let mut load = None;
    let mut bytes = None;
    let result = async {
        bytes = Some(
            protocol::asynchronous::send_proposal(
                &mut initial,
                protocol::MAX_MESSAGE_SIZE,
                Some(&task),
                None,
                proposal,
                |name| {
                    load.get_or_insert_with(|| {
                        crate::progress::load(source, "", &remote.name, None)
                    });
                    super::read_object(name)
                },
                |name, keys| super::certify(name, keys).ok(),
            )
            .await
            .map_err(|error| alloc::format!("program transfer failed: {error}"))?,
        );
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
        load.finish(result.is_ok(), bytes);
    }
    result
}

impl PendingTransfer {
    /// Import an accepted transfer, checking the expected result or evaluation coupon.
    pub async fn receive(mut self) -> Result<Value, Failure> {
        let trusted_keys = super::trusted_keys()?;
        let remote = self.peer.as_ref().ok_or(Failure::Closed)?.name.clone();
        let mut download = None;
        let mut bytes = 0;
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
                        download =
                            Some(crate::progress::load(&value, &remote, "", Some(peer.node)));
                    }
                    Some(missing)
                },
                None,
                |message| match message {
                    Message::EvaluationProgress(snapshot, _) => {
                        let _ = super::report_progress(&remote, snapshot, true);
                    }
                    Message::BlobData(record)
                    | Message::TreeData(record)
                    | Message::ShallowTreeData(record) => {
                        bytes += record.data.as_ref().map_or(0, |data| data.len() as u64);
                    }
                    _ => (),
                },
            )
            .await
            .map_err(Failure::Receive)?
            .ok_or(Failure::Closed)?;
            let (task, expected_name) = match &self.expected {
                Expected::Value(name) => (None, Some(*name)),
                Expected::Coupon { task, .. } => (Some(task), None),
            };
            if incoming.task.as_ref() != task
                || incoming.result.is_none()
                || expected_name.is_some_and(|name| incoming.result != Some(name))
            {
                return Err(Failure::Task);
            }
            let bytes = incoming.transfer.encode().map_err(|_| Failure::Task)?;
            let roots = super::import(&bytes).map_err(Failure::Import)?;
            let [value] = roots.as_slice() else {
                return Err(Failure::Coupon);
            };
            match &self.expected {
                Expected::Value(name) if value.raw_handle_bytes()? != *name => {
                    return Err(Failure::Task);
                }
                Expected::Coupon {
                    task,
                    input,
                    result,
                } => {
                    crate::collector::validate(value.clone(), task.operation, input)?
                        .ok_or(Failure::Coupon)?;
                    if let Some(expected) = result
                        && storage::canonical_value(&crate::collector::result(value)?)?
                            .raw_handle_bytes()?
                            != *expected
                    {
                        return Err(Failure::Coupon);
                    }
                }
                _ => {}
            }
            self.network.completed(peer.node, &self.names, task);

            Ok(value.clone())
        }
        .await;
        if result.is_ok()
            && let Some(peer) = &mut self.peer
        {
            peer.connection.finish_request();
        }
        if let Some(load) = download {
            load.finish(result.is_ok(), Some(bytes));
        }
        result
    }
}
