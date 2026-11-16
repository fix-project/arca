use super::{
    Connection, configured,
    protocol::{self, Information, Message, Task, Transfer},
};
use crate::{io::Io, kernel::worker, shell::Value, storage};
use alloc::{
    rc::{Rc, Weak},
    vec::Vec,
};
use core::cell::RefCell;
use fixwasm::{Error, Value as AsValue};

const MAX_TRANSFER: usize = 512 << 20;

struct Peer<'a> {
    connection: Connection<'a>,
    information: Information,
}

#[derive(Debug)]
pub enum Failure {
    Shell(Error),
    Export,
    Send(protocol::Error<super::IoError>),
    Receive(protocol::Error<super::IoError>),
    Closed,
    Task,
    Import,
    Coupon,
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self::Shell(error)
    }
}

pub struct Cluster<'a> {
    peers: Vec<Rc<RefCell<Option<Peer<'a>>>>>,
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
    pub async fn connect(io: &'a Io) -> Result<Self, Error> {
        let mut peers = Vec::new();
        for remote in configured()? {
            let peer = (async {
                let mut connection = Connection::connect_tcp(io, &remote).await.ok().flatten()?;
                if protocol::asynchronous::send(&mut connection, Message::RequestInfo)
                    .await
                    .is_err()
                {
                    drop(connection);
                    return None;
                }
                match protocol::asynchronous::receive(&mut connection, MAX_TRANSFER).await {
                    Ok(Some(Message::Info(information))) => Some(Peer {
                        connection,
                        information,
                    }),
                    _ => {
                        drop(connection);
                        None
                    }
                }
            })
            .await;
            peers.push(Rc::new(RefCell::new(peer)));
        }
        Ok(Self { peers })
    }

    pub fn information(&self) -> Vec<Information> {
        self.peers
            .iter()
            .map(|peer| {
                peer.borrow()
                    .as_ref()
                    .map_or_else(Information::default, |peer| peer.information.clone())
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
            super::export(&[collector.clone(), input.clone()], &[])?.ok_or(Failure::Export)?;
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
        protocol::asynchronous::send_transfer(
            &mut pending.peer.as_mut().unwrap().connection,
            MAX_TRANSFER,
            &pending.task,
            None,
            transfer,
            |name, keys| super::certify(name, keys).ok().flatten(),
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

impl PendingEvaluation<'_> {
    /// Wait for a submitted evaluation and return its validated coupon.
    pub async fn receive(mut self) -> Result<Value, Failure> {
        let trusted_keys = super::trusted_keys()?;
        let peer = self.peer.as_mut().ok_or(Failure::Closed)?;
        let incoming = protocol::asynchronous::receive_transfer(
            &mut peer.connection,
            MAX_TRANSFER,
            worker::count()?.try_into().map_err(|_| Failure::Task)?,
            &trusted_keys,
            |offers| super::missing(offers).ok().flatten(),
        )
        .await
        .map_err(Failure::Receive)?
        .ok_or(Failure::Closed)?;
        if incoming.task != self.task || incoming.result.is_none() {
            return Err(Failure::Task);
        }
        let bytes = incoming.transfer.encode().map_err(|_| Failure::Task)?;
        let roots = super::import(&bytes)?.ok_or(Failure::Import)?;
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
