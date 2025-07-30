use crate::prelude::*;
use common::util::channel::ChannelClosed;

pub mod addr;
pub(crate) mod driver;
pub mod flow;
pub mod header;
pub mod listener;
pub mod message;
pub mod stream;

pub use addr::*;
pub use driver::*;
pub use flow::*;
pub use header::*;
pub use listener::*;
pub use message::*;
pub use stream::*;

#[derive(Debug)]
pub enum SocketError {
    InvalidAddress(SocketAddr),
    AddressInUse(SocketAddr),
    ConnectionReset,
    ConnectionClosed,
    ConnectionFailed,
}

pub type Result<T> = core::result::Result<T, SocketError>;

impl From<ChannelClosed> for SocketError {
    fn from(_: ChannelClosed) -> Self {
        SocketError::ConnectionClosed
    }
}

pub(crate) static DRIVER: OnceLock<Arc<Driver>> = OnceLock::new();

pub(crate) async fn listen(addr: SocketAddr) -> Listener {
    let driver = &DRIVER;
    driver.listen(addr).await
}

pub(crate) async fn accept(flow: Flow) -> Receiver {
    let driver = &DRIVER;
    driver.accept(flow).await
}

pub(crate) async fn connect(flow: Flow) -> Receiver {
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
