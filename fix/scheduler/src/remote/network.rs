use super::{
    Connection, IoError, ProgressObserver, RawStream,
    protocol::{
        self, Information, Inventory, Message, Packet, WorkerCapacity,
        topology::{Advertisement, LinkId, Neighbor, NodeId, Route as Path, Topology},
    },
};
use crate::{
    abi::effect,
    io::{Background, Io, log},
};
use alloc::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    rc::{Rc, Weak},
    string::String,
    vec::Vec,
};
use core::{
    cell::RefCell,
    future::poll_fn,
    task::{Poll, Waker},
};
use fixwasm::Error;

struct Global(RefCell<Option<Rc<Network>>>);
// One kernel thread accesses a scheduler Wasm instance.
unsafe impl Sync for Global {}
static GLOBAL: Global = Global(RefCell::new(None));

pub(super) struct Network {
    node: NodeId,
    state: RefCell<State>,
    reporting: RefCell<Option<(Background, Background)>>,
    discovery: RefCell<Discovery>,
}
type PeerFailures = Rc<BTreeMap<String, String>>;

#[derive(Default)]
struct Discovery {
    task: Option<Background>,
    result: Option<Result<PeerFailures, Error>>,
    waiters: Vec<Waker>,
}

struct State {
    sequence: u64,
    next_link: u64,
    topology: Topology,
    paths: Vec<Path>,
    inventories: BTreeMap<NodeId, Rc<Inventory>>,
    capacity: BTreeMap<NodeId, (u64, WorkerCapacity)>,
    transfers: BTreeMap<NodeId, Transfers>,
    links: BTreeMap<LinkId, Rc<Link>>,
    incoming: Vec<Connection>,
    incoming_tasks: BTreeMap<(NodeId, TaskKey), usize>,
    outgoing_tasks: BTreeMap<TaskKey, usize>,
    waiters: Vec<Waker>,
}
type TaskKey = (u64, protocol::Name, protocol::Name);

#[derive(Default)]
struct Transfers {
    data: BTreeSet<protocol::Name>,
    results: BTreeSet<TaskKey>,
    coupons: BTreeMap<TaskKey, protocol::Name>,
}

pub(super) struct Route {
    pub path: Path,
    pub information: Information,
}

struct Link {
    network: Weak<Network>,
    id: LinkId,
    node: NodeId,
    name: String,
    raw: Rc<RawStream>,
    progress: ProgressObserver,
    state: RefCell<LinkState>,
    pumps: RefCell<Vec<Background>>,
}
struct LinkState {
    closed: bool,
    used: bool,
    next_request: u64,
    outgoing: VecDeque<Packet>,
    writer: Option<Waker>,
    closed_waiters: Vec<Waker>,
    requests: BTreeMap<u64, Rc<RefCell<RequestInbox>>>,
}
#[derive(Default)]
struct RequestInbox {
    messages: VecDeque<Message>,
    closed: bool,
    failure: Option<Rc<str>>,
    reader: Option<Waker>,
}

impl RequestInbox {
    fn close(&mut self, failure: Option<Rc<str>>) {
        self.closed = true;
        self.failure = failure;
        if let Some(reader) = self.reader.take() {
            reader.wake();
        }
    }
}

enum RequestState {
    Incoming(Option<TaskKey>),
    New,
    Waiting,
    Finished,
}

pub struct Request {
    link: Rc<Link>,
    id: u64,
    inbox: Rc<RefCell<RequestInbox>>,
    state: RequestState,
    delegated: Option<TaskKey>,
}

impl Request {
    pub(super) fn progress(&self) -> Result<Vec<u8>, Error> {
        self.link.progress.snapshot()
    }

    pub(super) fn mark_used(&self) {
        self.link.state.borrow_mut().used = true;
    }

    pub(super) fn finish(&mut self) {
        self.state = RequestState::Finished;
    }
}

impl Network {
    pub(super) fn get() -> Result<Rc<Self>, Error> {
        if let Some(network) = GLOBAL.0.borrow().as_ref() {
            return Ok(network.clone());
        }
        let node: NodeId = super::bytes(&effect(b"node_identity", &[])?)?
            .try_into()
            .map_err(|_| Error::WrongType)?;
        let network = Rc::new(Self {
            node,
            state: RefCell::new(State {
                sequence: 0,
                next_link: 0,
                topology: Topology::default(),
                paths: Vec::new(),
                inventories: BTreeMap::new(),
                capacity: BTreeMap::new(),
                transfers: BTreeMap::new(),
                links: BTreeMap::new(),
                incoming: Vec::new(),
                incoming_tasks: BTreeMap::new(),
                outgoing_tasks: BTreeMap::new(),
                waiters: Vec::new(),
            }),
            reporting: RefCell::default(),
            discovery: RefCell::default(),
        });
        network.publish();
        *GLOBAL.0.borrow_mut() = Some(network.clone());
        Ok(network)
    }

    fn start_discovery(self: &Rc<Self>, io: &Io) {
        let mut discovery = self.discovery.borrow_mut();
        if discovery.task.is_none() {
            let network = self.clone();
            discovery.task = Some(io.spawn(async move {
                let result = super::cluster::discover_peers(&network).await.map(Rc::new);
                let mut discovery = network.discovery.borrow_mut();
                discovery.result = Some(result);
                for waiter in discovery.waiters.drain(..) {
                    waiter.wake();
                }
            }));
        }
    }

    pub(super) async fn discover(self: &Rc<Self>, io: &Io) -> Result<PeerFailures, Error> {
        self.start_discovery(io);
        poll_fn(|cx| {
            let mut discovery = self.discovery.borrow_mut();
            if let Some(result) = &discovery.result {
                return Poll::Ready(result.clone());
            }
            if !discovery
                .waiters
                .iter()
                .any(|waker| waker.will_wake(cx.waker()))
            {
                discovery.waiters.push(cx.waker().clone());
            }
            Poll::Pending
        })
        .await
    }

    async fn load_inventory(&self) -> Result<(), Error> {
        let (status, value) = Io
            .request(fixwasm::tree![
                fixwasm::Blob::create(b"remote_inventory"),
                crate::collector::callable()?,
            ])
            .await?;
        if status != 0 {
            return Err(Error::WrongType);
        }
        let information =
            postcard::from_bytes(&super::bytes(&value)?).map_err(|_| Error::WrongType)?;
        self.record_inventory(self.node, information, None);
        Ok(())
    }

    fn record_inventory(&self, node: NodeId, mut information: Inventory, from: Option<LinkId>) {
        let mut state = self.state.borrow_mut();
        let alloc::collections::btree_map::Entry::Vacant(entry) = state.inventories.entry(node)
        else {
            return;
        };
        information.data.sort_unstable();
        information.results.sort_unstable_by_key(task_key);
        let information = entry.insert(Rc::new(information)).clone();
        for link in state.links.values().filter(|link| Some(link.id) != from) {
            link.enqueue(Packet::Inventory(node, (*information).clone()));
        }
        for waiter in state.waiters.drain(..) {
            waiter.wake();
        }
    }

    fn record_capacity(
        &self,
        node: NodeId,
        revision: u64,
        capacity: WorkerCapacity,
        from: Option<LinkId>,
    ) {
        let mut state = self.state.borrow_mut();
        if state
            .capacity
            .get(&node)
            .is_some_and(|(seen, _)| *seen >= revision)
        {
            return;
        }
        state.capacity.insert(node, (revision, capacity));
        for link in state.links.values().filter(|link| Some(link.id) != from) {
            link.enqueue(Packet::Capacity(node, revision, capacity));
        }
        for waiter in state.waiters.drain(..) {
            waiter.wake();
        }
    }

    fn sample_capacity(&self) -> Result<(), Error> {
        use crate::kernel::worker;
        let previous = self.state.borrow().capacity.get(&self.node).copied();
        let mut capacity = match previous {
            Some((_, capacity)) => capacity,
            None => WorkerCapacity {
                parallelism: worker::count()?.try_into()?,
                total_bytes: worker::memory_capacity()?,
                available_bytes: 0,
            },
        };
        capacity.available_bytes = worker::available_memory()?;
        if previous.is_none_or(|(_, old)| old != capacity) {
            self.record_capacity(
                self.node,
                previous.map_or(1, |(rev, _)| rev + 1),
                capacity,
                None,
            );
        }
        Ok(())
    }

    fn start_reporting(self: &Rc<Self>) {
        let mut reporting = self.reporting.borrow_mut();
        if reporting.is_some() {
            return;
        }
        let network = Rc::downgrade(self);
        let inventory = Io.spawn(async move {
            if let Some(network) = network.upgrade()
                && network.load_inventory().await.is_err()
            {
                let links: Vec<_> = network.state.borrow().links.values().cloned().collect();
                for link in links {
                    network.disconnect(&link, None);
                }
                network.reporting.borrow_mut().take();
            }
        });
        let network = Rc::downgrade(self);
        let progress = Io.spawn(async move {
            loop {
                let Some(network) = network.upgrade() else {
                    return;
                };
                let links: Vec<_> = network
                    .state
                    .borrow()
                    .links
                    .values()
                    .filter(|link| !link.state.borrow().closed)
                    .cloned()
                    .collect();
                if !links.is_empty() {
                    let _ = network.sample_capacity();
                    for link in links {
                        if let Ok(snapshot) = link.progress.snapshot() {
                            link.enqueue(Packet::Progress(snapshot));
                        }
                    }
                }
                if Io
                    .request(fixwasm::tree![fixwasm::Blob::create(b"sleep"), 1000u64])
                    .await
                    .is_err()
                {
                    return;
                }
            }
        });
        *reporting = Some((inventory, progress));
    }

    fn publish(&self) {
        let mut state = self.state.borrow_mut();
        state.sequence = state
            .sequence
            .checked_add(1)
            .expect("topology revision exhausted");
        let advertisement = Advertisement {
            node: self.node,
            sequence: state.sequence,
            neighbors: state
                .links
                .values()
                .filter(|link| !link.state.borrow().closed)
                .map(|link| Neighbor {
                    link: link.id,
                    node: link.node,
                    name: link.name.clone(),
                })
                .collect(),
        };
        state.topology.update(advertisement.clone());
        state.paths = state.topology.routes(self.node);
        let mut update = Topology::default();
        update.update(advertisement);
        for link in state.links.values() {
            link.enqueue(Packet::Topology(update.clone()));
        }
    }

    pub(super) fn routes(&self, collector: Option<protocol::Name>) -> Vec<Route> {
        let state = self.state.borrow();
        state
            .paths
            .iter()
            .filter_map(|path| {
                let inventory = state.inventories.get(&path.node)?;
                let &(_, capacity) = state.capacity.get(&path.node)?;
                let mut information = Information {
                    parallelism: capacity.parallelism,
                    memory_bytes: Some(capacity.available_bytes),
                    memory_capacity_bytes: Some(capacity.total_bytes),
                    data: inventory.data.clone(),
                    results: inventory.results.clone(),
                    ..Information::default()
                };
                if let Some(transfers) = state.transfers.get(&path.node) {
                    information.data.extend(transfers.data.iter().copied());
                    information.results.extend(transfers.results.iter().map(
                        |&(operation, input, collector)| protocol::Task {
                            operation,
                            input,
                            collector,
                        },
                    ));
                    if let Some(collector) = collector {
                        let kernel = *fixhandle::Tree::empty().as_handle().as_bytes();
                        information
                            .results
                            .extend(transfers.coupons.keys().filter_map(
                                |&(operation, input, author)| {
                                    (author == collector || author == kernel).then_some(
                                        protocol::Task {
                                            operation,
                                            input,
                                            collector,
                                        },
                                    )
                                },
                            ));
                    }
                }
                Some(Route {
                    path: path.clone(),
                    information,
                })
            })
            .collect()
    }

    pub(super) fn available_routes(&self, required: u64) -> Vec<(Path, u64)> {
        let state = self.state.borrow();
        state
            .paths
            .iter()
            .filter_map(|path| {
                let &(_, capacity) = state.capacity.get(&path.node)?;
                let slots = (capacity.available_bytes / required.max(1))
                    .min(u64::from(capacity.parallelism));
                (slots > 0).then(|| (path.clone(), slots))
            })
            .collect()
    }

    pub(super) fn incoming(&self) -> Vec<Connection> {
        core::mem::take(&mut self.state.borrow_mut().incoming)
    }

    pub(super) async fn connect(
        self: &Rc<Self>,
        mut stream: Connection,
        name: String,
    ) -> Result<(), IoError> {
        let id = {
            let mut state = self.state.borrow_mut();
            let id = LinkId {
                initiator: self.node,
                sequence: state.next_link,
            };
            state.next_link = state.next_link.checked_add(1).ok_or(IoError)?;
            id
        };
        protocol::asynchronous::send(
            &mut stream,
            Message::Hello {
                link: id,
                node: self.node,
            },
        )
        .await
        .map_err(|_| IoError)?;
        let Some(Message::HelloAccepted(node)) =
            protocol::asynchronous::receive(&mut stream, protocol::MAX_MESSAGE_SIZE)
                .await
                .map_err(|_| IoError)?
        else {
            return Err(IoError);
        };
        let link = self.attach(stream, id, node, name, 0)?;
        poll_fn(|cx| {
            let mut state = self.state.borrow_mut();
            if link.state.borrow().closed {
                Poll::Ready(Err(IoError))
            } else if state.inventories.contains_key(&node)
                && state.capacity.contains_key(&node)
                && state.paths.iter().any(|route| route.node == node)
            {
                Poll::Ready(Ok(()))
            } else {
                if !state.waiters.iter().any(|w| w.will_wake(cx.waker())) {
                    state.waiters.push(cx.waker().clone());
                }
                Poll::Pending
            }
        })
        .await?;
        if let Some(route) = self
            .routes(None)
            .into_iter()
            .find(|route| route.path.node == node)
        {
            super::report_node_status(
                &link.name,
                "available",
                Some(&route.information),
                "",
                Some(node),
            );
        }
        Ok(())
    }

    pub(super) async fn accept(
        self: &Rc<Self>,
        mut stream: Connection,
        id: LinkId,
        node: NodeId,
    ) -> Result<(), IoError> {
        if id.initiator != node {
            return Err(IoError);
        }
        protocol::asynchronous::send(&mut stream, Message::HelloAccepted(self.node))
            .await
            .map_err(|_| IoError)?;
        let link = self.attach(stream, id, node, String::new(), 1)?;
        self.start_discovery(&Io);
        poll_fn(|cx| {
            let mut state = link.state.borrow_mut();
            if state.closed {
                Poll::Ready(())
            } else {
                if !state.closed_waiters.iter().any(|w| w.will_wake(cx.waker())) {
                    state.closed_waiters.push(cx.waker().clone());
                }
                Poll::Pending
            }
        })
        .await;
        Ok(())
    }

    fn attach(
        self: &Rc<Self>,
        stream: Connection,
        id: LinkId,
        node: NodeId,
        name: String,
        first_request: u64,
    ) -> Result<Rc<Link>, IoError> {
        let Connection::Raw(raw) = stream else {
            return Err(IoError);
        };
        if !name.is_empty() {
            super::report_node_status(&name, "connecting", None, "", Some(node));
        }
        if self.state.borrow().links.contains_key(&id) {
            return Err(IoError);
        }
        let link = Rc::new(Link {
            network: Rc::downgrade(self),
            id,
            node,
            name,
            raw,
            progress: ProgressObserver::new().map_err(|_| IoError)?,
            pumps: RefCell::new(Vec::new()),
            state: RefCell::new(LinkState {
                closed: false,
                used: false,
                next_request: first_request,
                outgoing: VecDeque::new(),
                writer: None,
                closed_waiters: Vec::new(),
                requests: BTreeMap::new(),
            }),
        });
        self.state.borrow_mut().links.insert(id, link.clone());
        {
            let state = self.state.borrow();
            for (&node, inventory) in &state.inventories {
                link.enqueue(Packet::Inventory(node, inventory.as_ref().clone()));
            }
            for (&node, &(revision, capacity)) in &state.capacity {
                link.enqueue(Packet::Capacity(node, revision, capacity));
            }
        }
        self.publish();
        link.enqueue(Packet::Topology(self.state.borrow().topology.clone()));
        self.start_reporting();
        let network = Rc::downgrade(self);
        let reading = Rc::downgrade(&link);
        let raw = link.raw.clone();
        link.pumps.borrow_mut().push(Io.spawn(async move {
            let mut stream = raw.as_ref();
            loop {
                let frame =
                    protocol::asynchronous::receive_packet_with_payload_limit(
                        &mut stream,
                        protocol::MAX_MESSAGE_SIZE,
                        || super::transfer_limit().unwrap_or(0),
                    )
                    .await;
                let (Some(network), Some(link)) = (network.upgrade(), reading.upgrade()) else {
                    return;
                };
                if link.state.borrow().closed {
                    return;
                }
                let reason = match frame {
                    Ok(Some(frame)) => {
                        if network.receive(&link, frame).is_ok() {
                            continue;
                        }
                        Some(String::from("invalid job packet"))
                    }
                    Ok(None) => None,
                    Err(error) => Some(alloc::format!("{error}")),
                };
                if let Some(reason) = &reason {
                    let _ = log(&alloc::format!("remote link {}: {reason}", link.name));
                }
                network.disconnect(&link, reason.map(Rc::from));
                return;
            }
        }));
        let network = Rc::downgrade(self);
        let writing = Rc::downgrade(&link);
        let raw = link.raw.clone();
        link.pumps.borrow_mut().push(Io.spawn(async move {
            let mut stream = raw.as_ref();
            loop {
                let frame = poll_fn(|cx| {
                    let Some(link) = writing.upgrade() else {
                        return Poll::Ready(None);
                    };
                    let mut state = link.state.borrow_mut();
                    if state.closed {
                        Poll::Ready(None)
                    } else if let Some(frame) = state.outgoing.pop_front() {
                        Poll::Ready(Some(frame))
                    } else {
                        state.writer = Some(cx.waker().clone());
                        Poll::Pending
                    }
                })
                .await;
                let Some(frame) = frame else {
                    return;
                };
                if let Err(error) = protocol::asynchronous::send_packet(&mut stream, frame).await {
                    if let (Some(network), Some(link)) = (network.upgrade(), writing.upgrade()) {
                        network.disconnect(&link, Some(Rc::from(alloc::format!("{error}"))));
                    }
                    return;
                }
            }
        }));
        Ok(link)
    }

    fn disconnect(&self, link: &Rc<Link>, failure: Option<Rc<str>>) {
        {
            let mut state = link.state.borrow_mut();
            if state.closed {
                return;
            }
            state.closed = true;
            if let Some(w) = state.writer.take() {
                w.wake();
            }
            for w in state.closed_waiters.drain(..) {
                w.wake();
            }
            for inbox in state.requests.values() {
                inbox.borrow_mut().close(failure.clone());
            }
        }
        link.raw.close();
        self.publish();
        for w in self.state.borrow_mut().waiters.drain(..) {
            w.wake();
        }
    }

    fn receive(&self, link: &Rc<Link>, frame: Packet) -> Result<(), IoError> {
        match frame {
            Packet::Progress(snapshot) => {
                let name = self
                    .state
                    .borrow()
                    .paths
                    .iter()
                    .find(|route| route.node == link.node)
                    .map(|route| route.name.clone())
                    .unwrap_or_else(|| link.name.clone());
                let used = link.state.borrow().used;
                let _ = super::report_progress(&name, &snapshot, used);
            }
            Packet::Topology(topology) => {
                let mut state = self.state.borrow_mut();
                if let Some(updates) = state.topology.merge_updates(topology) {
                    state.paths = state.topology.routes(self.node);
                    for w in state.waiters.drain(..) {
                        w.wake();
                    }
                    for other in state.links.values().filter(|other| other.id != link.id) {
                        other.enqueue(Packet::Topology(updates.clone()));
                    }
                }
            }
            Packet::Inventory(node, information) => {
                if node != self.node {
                    self.record_inventory(node, information, Some(link.id));
                }
            }
            Packet::Capacity(node, revision, capacity) => {
                if node != self.node {
                    self.record_capacity(node, revision, capacity, Some(link.id));
                }
            }
            Packet::Start(id, message) => {
                let mut state = link.state.borrow_mut();
                if id % 2 == state.next_request % 2 || state.requests.contains_key(&id) {
                    return Err(IoError);
                }
                let task = proposed_task(&message);
                if task.is_some_and(|task| self.state.borrow().outgoing_tasks.contains_key(&task)) {
                    drop(state);
                    link.enqueue(Packet::Job(
                        id,
                        Message::Failure("worker is already awaiting this evaluation".into()),
                    ));
                    return Ok(());
                }
                self.record_proposal(link.node, &message);
                let inbox = Rc::new(RefCell::new(RequestInbox {
                    messages: VecDeque::from([message]),
                    ..RequestInbox::default()
                }));
                state.requests.insert(id, inbox.clone());
                let mut network = self.state.borrow_mut();
                if let Some(task) = task {
                    *network.incoming_tasks.entry((link.node, task)).or_default() += 1;
                }
                network.incoming.push(Connection::Request(Request {
                    link: link.clone(),
                    id,
                    inbox,
                    state: RequestState::Incoming(task),
                    delegated: None,
                }));
            }
            Packet::Job(id, message) => {
                let state = link.state.borrow();
                if let Some(inbox) = state.requests.get(&id) {
                    self.record_proposal(link.node, &message);
                    let mut inbox = inbox.borrow_mut();
                    inbox.messages.push_back(message);
                    if let Some(waker) = inbox.reader.take() {
                        waker.wake();
                    }
                }
            }
            Packet::Stop(id) => {
                let state = link.state.borrow();
                if id % 2 == state.next_request % 2 {
                    return Err(IoError);
                }
                if let Some(inbox) = state.requests.get(&id) {
                    let mut inbox = inbox.borrow_mut();
                    inbox.messages.clear();
                    inbox.close(None);
                }
            }
        }
        Ok(())
    }

    pub(super) fn is_direct(&self, node: NodeId) -> bool {
        self.state
            .borrow()
            .links
            .values()
            .any(|link| link.node == node && !link.state.borrow().closed)
    }

    pub(super) fn open(
        &self,
        destination: NodeId,
    ) -> Result<(Connection, NodeId, String), IoError> {
        let (link, name) = {
            let state = self.state.borrow();
            let direct = state
                .links
                .values()
                .filter(|link| link.node == destination && !link.state.borrow().closed)
                .min_by_key(|link| (link.name.is_empty(), &link.name, link.id));
            if let Some(link) = direct {
                let name = if link.name.is_empty() {
                    destination
                        .iter()
                        .map(|byte| alloc::format!("{byte:02x}"))
                        .collect()
                } else {
                    link.name.clone()
                };
                (link.clone(), name)
            } else {
                let route = state
                    .paths
                    .iter()
                    .find(|route| route.node == destination)
                    .ok_or(IoError)?;
                let link = state.links.get(&route.next).cloned().ok_or(IoError)?;
                let name = route
                    .name
                    .split_once('!')
                    .map_or(route.name.as_str(), |(first, _)| first)
                    .into();
                (link, name)
            }
        };
        let (id, inbox) = {
            let mut state = link.state.borrow_mut();
            if state.closed {
                return Err(IoError);
            }
            let id = state.next_request;
            state.next_request = id.checked_add(2).ok_or(IoError)?;
            let inbox = Rc::new(RefCell::new(RequestInbox::default()));
            state.requests.insert(id, inbox.clone());
            (id, inbox)
        };
        let node = link.node;
        Ok((
            Connection::Request(Request {
                link,
                id,
                inbox,
                state: RequestState::New,
                delegated: None,
            }),
            node,
            name,
        ))
    }

    pub(super) fn awaiting_result(&self, node: NodeId, task: &protocol::Task) -> bool {
        self.state
            .borrow()
            .incoming_tasks
            .contains_key(&(node, task_key(task)))
    }

    pub(super) fn completed(
        &self,
        node: NodeId,
        names: &[protocol::Name],
        task: Option<&protocol::Task>,
    ) {
        // Successful transfers are evidence about the immediate recipient only.
        let mut state = self.state.borrow_mut();
        let transfers = state.transfers.entry(node).or_default();
        transfers.data.extend(names.iter().copied());
        if let Some(task) = task {
            transfers.results.insert(task_key(task));
        }
    }

    pub(super) fn coupon(&self, node: NodeId, task: &protocol::Task) -> Option<protocol::Name> {
        let state = self.state.borrow();
        let coupons = &state.transfers.get(&node)?.coupons;
        coupons
            .get(&task_key(task))
            .or_else(|| {
                coupons.get(&(
                    task.operation,
                    task.input,
                    *fixhandle::Tree::empty().as_handle().as_bytes(),
                ))
            })
            .copied()
    }

    pub(super) fn offers_coupons(&self, node: NodeId) -> bool {
        self.state
            .borrow()
            .transfers
            .get(&node)
            .is_some_and(|transfer| !transfer.coupons.is_empty())
    }

    fn record_proposal(&self, node: NodeId, message: &Message) {
        let Message::ProposeTransfer {
            objects, coupons, ..
        } = message
        else {
            return;
        };
        let names = objects.iter().filter_map(|offer| {
            if offer.backing
                && offer.is_ref()
                && let Ok(handle) = fixhandle::Handle::parse_ref(&offer.name)
                && handle.is_canonical()
            {
                let mut object = offer.name;
                object[31] &= 0xe0;
                Some(object)
            } else {
                None
            }
        });
        let mut names = names.peekable();
        if names.peek().is_some() || !coupons.is_empty() {
            let mut state = self.state.borrow_mut();
            let transfers = state.transfers.entry(node).or_default();
            transfers.data.extend(names);
            for coupon in coupons {
                if protocol::CouponOffer::supports(coupon.operation)
                    && [&coupon.input, &coupon.result, &coupon.author]
                        .into_iter()
                        .all(|name| {
                            fixhandle::Handle::parse_ref(name).is_ok_and(|name| name.is_canonical())
                        })
                {
                    transfers.coupons.insert(
                        (coupon.operation, coupon.input, coupon.author),
                        coupon.result,
                    );
                }
            }
        }
    }
}

fn task_key(task: &protocol::Task) -> TaskKey {
    (task.operation, task.input, task.collector)
}

fn proposed_task(message: &Message) -> Option<TaskKey> {
    match message {
        Message::ProposeTransfer {
            task: Some(task),
            result: None,
            ..
        } => Some(task_key(task)),
        _ => None,
    }
}

impl Link {
    fn enqueue(&self, frame: Packet) {
        let mut state = self.state.borrow_mut();
        if state.closed {
            return;
        }
        match &frame {
            Packet::Topology(update) => {
                if let Some(queued) = state.outgoing.iter_mut().find_map(|frame| match frame {
                    Packet::Topology(topology) => Some(topology),
                    _ => None,
                }) {
                    queued.merge(update);
                    return;
                }
            }
            Packet::Capacity(node, ..) => state
                .outgoing
                .retain(|frame| !matches!(frame, Packet::Capacity(queued, ..) if queued == node)),
            _ => (),
        }
        state.outgoing.push_back(frame);
        if let Some(w) = state.writer.take() {
            w.wake();
        }
    }
}

impl Request {
    pub(super) async fn read(&self) -> Result<Option<Message>, protocol::Error<IoError>> {
        poll_fn(|cx| {
            let mut inbox = self.inbox.borrow_mut();
            if let Some(packet) = inbox.messages.pop_front() {
                Poll::Ready(Ok(Some(packet)))
            } else if inbox.closed {
                Poll::Ready(match &inbox.failure {
                    Some(reason) => Err(protocol::Error::Connection(String::from(reason.as_ref()))),
                    None => Ok(None),
                })
            } else {
                inbox.reader = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await
    }

    pub(super) fn write(&mut self, message: Message) -> Result<(), IoError> {
        if self.inbox.borrow().closed {
            return Err(IoError);
        }
        let frame = if matches!(self.state, RequestState::New) {
            if let Some(task) = proposed_task(&message)
                && let Some(network) = self.link.network.upgrade()
            {
                *network
                    .state
                    .borrow_mut()
                    .outgoing_tasks
                    .entry(task)
                    .or_default() += 1;
                self.delegated = Some(task);
            }
            self.state = RequestState::Waiting;
            Packet::Start(self.id, message)
        } else {
            Packet::Job(self.id, message)
        };
        self.link.enqueue(frame);
        Ok(())
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        self.link.state.borrow_mut().requests.remove(&self.id);
        if let RequestState::Incoming(Some(task)) = self.state
            && let Some(network) = self.link.network.upgrade()
        {
            let mut network = network.state.borrow_mut();
            let key = (self.link.node, task);
            let count = network.incoming_tasks.get_mut(&key).unwrap();
            *count -= 1;
            if *count == 0 {
                network.incoming_tasks.remove(&key);
            }
        }
        if let Some(task) = self.delegated
            && let Some(network) = self.link.network.upgrade()
        {
            let mut state = network.state.borrow_mut();
            let count = state.outgoing_tasks.get_mut(&task).unwrap();
            *count -= 1;
            if *count == 0 {
                state.outgoing_tasks.remove(&task);
            }
        }
        if matches!(self.state, RequestState::Waiting) {
            self.link.enqueue(Packet::Stop(self.id));
        }
    }
}
