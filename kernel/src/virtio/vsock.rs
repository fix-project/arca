use crate::prelude::*;

pub mod addr;
pub(crate) mod driver;
pub mod flow;
pub mod header;
pub mod listener;
pub mod message;
pub mod stream;

pub use addr::*;
use common::util::channel::{RecvError, SendError};
pub use driver::*;
pub use flow::*;
pub use header::*;
pub use listener::*;
pub use message::*;
pub use stream::*;

use async_lock::RwLock;

#[derive(Debug)]
pub enum SocketError {
    InvalidAddress,
    AddressInUse(SocketAddr),
    ConnectionReset,
    ConnectionClosed,
    ConnectionFailed,
}

pub type Result<T> = core::result::Result<T, SocketError>;

impl<T> From<SendError<T>> for SocketError {
    fn from(_: SendError<T>) -> Self {
        SocketError::ConnectionClosed
    }
}

impl From<RecvError> for SocketError {
    fn from(_: RecvError) -> Self {
        SocketError::ConnectionClosed
    }
}

pub(crate) static DRIVER: OnceLock<Arc<Driver>> = OnceLock::new();

pub(crate) async fn listen(addr: SocketAddr) -> Arc<RwLock<ListenSocket>> {
    let driver = &DRIVER;
    driver.listen(addr).await
}

pub(crate) async fn accept(flow: Flow) -> Arc<RwLock<StreamSocket>> {
    let driver = &DRIVER;
    driver.accept(flow).await
}

pub(crate) async fn connect(flow: Flow) -> Arc<RwLock<StreamSocket>> {
    let driver = &DRIVER;
    driver.connect(flow).await
}

pub(crate) async fn send(flow: Flow, buf: &[u8]) -> usize {
    let driver = &DRIVER;
    driver.send(flow, buf).await
}

pub(crate) async fn shutdown(flow: Flow, rx: bool, tx: bool) {
    let driver = &DRIVER;
    driver.shutdown(flow, rx, tx).await
}

pub(crate) async fn rst(flow: Flow) {
    let driver = &DRIVER;
    driver.rst(flow).await
}

#[allow(unused)]
pub(crate) async fn listeners() -> Vec<SocketAddr> {
    let driver = &DRIVER;
    driver.listeners().await
}

#[allow(unused)]
pub(crate) async fn streams() -> Vec<Flow> {
    let driver = &DRIVER;
    driver.streams().await
}
