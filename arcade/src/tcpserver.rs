use core::panic;

use async_trait::async_trait;
use kernel::host::*;
use kernel::prelude::*;
use serde::{Deserialize, Serialize};
use vfs::{File, Open};

use crate::proc::Namespace;

#[derive(Debug)]
pub enum Error {
    Eof,
    Vfs,
    MessageProcessing,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FileRequest {
    pub file_path: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Continuation {
    pub data: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FileResponse {
    pub file_data: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Message {
    FileRequest(FileRequest),
    Continuation(Continuation),
    FileResponse(FileResponse),
    ClientClose,
}

async fn write_all(f: fn(*const [u8]) -> usize, mut buf: &[u8]) -> Result<(), Error> {
    while !buf.is_empty() {
        let n = f(buf);
        buf = &buf[n..];
    }
    Ok(())
}

async fn write_message_to_f(f: fn(*const [u8]) -> usize, message: &Message) -> Result<(), Error> {
    let m = postcard::to_allocvec(message).unwrap();
    let buf = m.len().to_ne_bytes();
    write_all(f, &buf).await?;
    write_all(f, m.as_slice()).await?;
    Ok(())
}

async fn read_exact(f: fn(*mut [u8]) -> usize, mut buf: &mut [u8]) -> Result<(), Error> {
    while !buf.is_empty() {
        let n = f(buf);
        buf = &mut buf[n..]
    }
    Ok(())
}

#[async_trait]
pub trait MessageServer {
    async fn process_message(&mut self, msg: Message) -> Result<(), Error>;
    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error>;
}

pub struct AblatedServer {
    shared_ns: Arc<Namespace>,
}

#[async_trait]
impl MessageServer for AblatedServer {
    async fn process_message(&mut self, msg: Message) -> Result<(), Error> {
        match msg {
            Message::FileRequest(FileRequest { file_path }) => {
                log::debug!("Received File Request for path {}", file_path);
                // send back the requested file data
                let mut file = self
                    .shared_ns
                    .walk(&file_path, Open::Read)
                    .await
                    .unwrap()
                    .as_file()
                    .unwrap();

                // TODO(kmohr) let's just encode the file size instead of reading in chunks like this
                let mut file_data = Vec::new();
                let mut buffer = [0u8; 4 * 1024 * 1024];
                loop {
                    match file.read(&mut buffer).await {
                        Ok(0) => break, // EOF
                        Ok(n) => file_data.extend_from_slice(&buffer[..n]),
                        Err(e) => {
                            log::error!("Failed to read file: {:?}", e);
                            break;
                        }
                    }
                }

                log::debug!("Read data {}", file_path);

                let response = Message::FileResponse(FileResponse { file_data });
                write_message_to_f(server_write, &response).await?;
                log::debug!("Replied {}", file_path);
                Ok(())
            }
            Message::ClientClose => Err(Error::MessageProcessing),
            Message::FileResponse(_) => {
                panic!("FileResponse should be handled AblatedClient")
            }
            Message::Continuation(_) => {
                panic!("Should not receive continuation message in ablated handler")
            }
        }
    }

    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        read_exact(server_read, buf).await
    }
}
impl AblatedServer {
    pub fn new(shared_ns: Arc<Namespace>) -> Self {
        AblatedServer { shared_ns }
    }
}

pub struct AblatedClientTx {
    // f: Arc<SpinLock<Box<dyn File>>>,
    // Futures to get some one to send the file request
    file_request_future_send: channel::Sender<Option<String>>,
    // Futures to be filled whe data arrives
    future_send: channel::Sender<Option<oneshot::Sender<Vec<u8>>>>,
}

pub struct AblatedClientRelay {
    file_request_future_recv: channel::Receiver<Option<String>>,
}

pub struct AblatedClientRx {
    future_recv: channel::Receiver<Option<oneshot::Sender<Vec<u8>>>>,
}

impl AblatedClientTx {
    fn new(
        file_request_future_send: channel::Sender<Option<String>>,
        future_send: channel::Sender<Option<oneshot::Sender<Vec<u8>>>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            file_request_future_send,
            future_send,
        })
    }

    pub async fn request_file(
        &self,
        file_path: String,
    ) -> Result<oneshot::Receiver<Vec<u8>>, Error> {
        let (sender, receiver) = oneshot::channel();
        self.future_send
            .send_blocking(Some(sender))
            .expect("Failed to send blocking");
        self.file_request_future_send
            .send_blocking(Some(file_path))
            .expect("Failed to send blocking");
        Ok(receiver)
    }

    pub async fn close(&self) {
        {
            self.future_send
                .send_blocking(None)
                .expect("Failed to send blocking");
            self.file_request_future_send
                .send_blocking(None)
                .expect("Failed to send blocking");
        }
    }
}

impl AblatedClientRelay {
    fn new(file_request_future_recv: channel::Receiver<Option<String>>) -> Self {
        Self {
            file_request_future_recv,
        }
    }

    async fn close(&mut self) {
        let _ = write_message_to_f(client_write, &Message::ClientClose).await;
    }

    pub async fn run(mut self) -> Result<(), Error> {
        loop {
            let x = self.file_request_future_recv.recv().await;
            match x {
                Err(_) => {
                    self.close().await;
                    return Ok(());
                }
                Ok(None) => {
                    self.close().await;
                    return Ok(());
                }
                Ok(Some(file_path)) => {
                    let m = Message::FileRequest(FileRequest { file_path });
                    let _ = write_message_to_f(client_write, &m).await;
                }
            }
        }
    }
}

impl AblatedClientRx {
    fn new(future_recv: channel::Receiver<Option<oneshot::Sender<Vec<u8>>>>) -> Self {
        Self { future_recv }
    }

    pub async fn run(self) -> Result<(), Error> {
        loop {
            let x = self.future_recv.recv().await;

            match x {
                Err(_) => {
                    log::debug!("ClientTx hanging up");
                    return Ok(());
                }
                Ok(None) => {
                    log::debug!("ClientTx hanging up");
                    return Ok(());
                }
                Ok(Some(future)) => {
                    let msg: Message = {
                        let mut buf = [0u8; 8];
                        read_exact(client_read, buf.as_mut_slice()).await.unwrap();
                        let len = usize::from_ne_bytes(buf);

                        let mut message_buf = vec![0u8; len];
                        read_exact(client_read, message_buf.as_mut_slice())
                            .await
                            .unwrap();
                        log::debug!("Read msg content");

                        postcard::from_bytes(message_buf.as_slice()).unwrap()
                    };

                    match msg {
                        Message::FileResponse(FileResponse { file_data }) => {
                            log::debug!("Received File Response");
                            future.send(file_data);
                        }
                        Message::ClientClose => panic!(),
                        Message::FileRequest(_) => panic!(),
                        Message::Continuation(_) => panic!(),
                    }
                }
            }
        }
    }
}

pub struct ContinuationServer {
    continuations: channel::Sender<Option<Vec<u8>>>,
}

#[async_trait]
impl MessageServer for ContinuationServer {
    async fn process_message(&mut self, msg: Message) -> Result<(), Error> {
        match msg {
            Message::Continuation(Continuation { data }) => {
                self.continuations
                    .send_blocking(Some(data))
                    .expect("Failed to send continuation data");
                Ok(())
            }
            Message::ClientClose => Err(Error::MessageProcessing),
            Message::FileResponse(_) => {
                panic!("Should not receive File Response in ContinuationHandler")
            }
            Message::FileRequest(_) => {
                panic!("Should not receive File Request in ContinuationHandler")
            }
        }
    }

    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        read_exact(server_read, buf).await
    }
}

impl Drop for ContinuationServer {
    fn drop(&mut self) {
        let _ = self.continuations.send_blocking(None);
    }
}

impl ContinuationServer {
    pub fn new(continuations: channel::Sender<Option<Vec<u8>>>) -> ContinuationServer {
        ContinuationServer { continuations }
    }
}

pub struct ContinuationClientRx;

impl ContinuationClientRx {
    fn new() -> Self {
        Self {}
    }

    pub async fn run(self) -> Result<(), Error> {
        Ok(())
    }
}

pub struct ContinuationClientTx {
    continuation_request_future_send: channel::Sender<Option<Vec<u8>>>,
}

impl ContinuationClientTx {
    fn new(continuation_request_future_send: channel::Sender<Option<Vec<u8>>>) -> Arc<Self> {
        Arc::new(Self {
            continuation_request_future_send,
        })
    }

    pub async fn request_to_run(&self, data: Vec<u8>) -> Result<(), Error> {
        let _ = self
            .continuation_request_future_send
            .send_blocking(Some(data));
        Ok(())
    }

    pub async fn close(&self) {
        {
            self.continuation_request_future_send
                .send_blocking(None)
                .expect("Failed to send blocking");
        }
    }
}

pub struct ContinuationClientRelay {
    continuation_request_future_recv: channel::Receiver<Option<Vec<u8>>>,
}

impl ContinuationClientRelay {
    fn new(continuation_request_future_recv: channel::Receiver<Option<Vec<u8>>>) -> Self {
        Self {
            continuation_request_future_recv,
        }
    }

    async fn close(&mut self) {
        let _ = write_message_to_f(client_write, &Message::ClientClose).await;
    }

    pub async fn run(mut self) -> Result<(), Error> {
        loop {
            let x = self.continuation_request_future_recv.recv().await;
            match x {
                Err(_) => {
                    self.close().await;
                    return Ok(());
                }
                Ok(None) => {
                    self.close().await;
                    return Ok(());
                }
                Ok(Some(data)) => {
                    let m = Message::Continuation(Continuation { data });
                    let _ = write_message_to_f(client_write, &m).await;
                }
            }
        }
    }
}

pub struct TcpServer<H: MessageServer + Send> {
    handler: H,
}

impl<H: MessageServer + Send> TcpServer<H> {
    pub fn new(handler: H) -> Self {
        Self { handler }
    }

    pub async fn run(mut self) -> Result<(), Error> {
        loop {
            let msg = {
                let mut buf = [0u8; 8];
                self.handler.read_exact(&mut buf).await?;
                let len = usize::from_ne_bytes(buf);
                let mut message_buf = vec![0u8; len];
                self.handler.read_exact(&mut message_buf).await?;
                postcard::from_bytes(message_buf.as_slice()).unwrap()
            };
            self.handler.process_message(msg).await?;
        }
    }
}

#[cfg(feature = "ablation")]
pub type ClientTx = AblatedClientTx;
#[cfg(feature = "ablation")]
pub type ClientRelay = AblatedClientRelay;
#[cfg(feature = "ablation")]
pub type ClientRx = AblatedClientRx;
#[cfg(not(feature = "ablation"))]
pub type ClientTx = ContinuationClientTx;
#[cfg(not(feature = "ablation"))]
pub type ClientRelay = ContinuationClientRelay;
#[cfg(not(feature = "ablation"))]
pub type ClientRx = ContinuationClientRx;

fn make_ablated_client() -> (Arc<AblatedClientTx>, AblatedClientRelay, AblatedClientRx) {
    let (file_request_future_send, file_request_future_recv) = channel::unbounded();
    let (future_send, future_recv) = channel::unbounded();

    (
        AblatedClientTx::new(file_request_future_send, future_send),
        AblatedClientRelay::new(file_request_future_recv),
        AblatedClientRx::new(future_recv),
    )
}

fn make_non_ablated_client() -> (
    Arc<ContinuationClientTx>,
    ContinuationClientRelay,
    ContinuationClientRx,
) {
    let (future_send, future_recv) = channel::unbounded();

    (
        ContinuationClientTx::new(future_send),
        ContinuationClientRelay::new(future_recv),
        ContinuationClientRx::new(),
    )
}

pub fn make_client() -> (Arc<ClientTx>, ClientRelay, ClientRx) {
    #[cfg(feature = "ablation")]
    let res = make_ablated_client();

    #[cfg(not(feature = "ablation"))]
    let res = make_non_ablated_client();

    res
}
