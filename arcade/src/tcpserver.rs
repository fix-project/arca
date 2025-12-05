use core::panic;

use async_trait::async_trait;
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

async fn write_all(f: &mut Box<dyn File>, mut buf: &[u8]) -> Result<(), Error> {
    while !buf.is_empty() {
        match f.write(buf).await {
            Ok(0) => return Err(Error::Vfs),
            Ok(n) => {
                buf = &buf[n..];
            }
            Err(_) => {
                return Err(Error::Vfs);
            }
        }
    }
    Ok(())
}

async fn write_message_to_f(f: &mut Box<dyn File>, message: &Message) -> Result<(), Error> {
    let m = postcard::to_allocvec(message).unwrap();
    let buf = m.len().to_ne_bytes();
    write_all(f, &buf).await?;
    write_all(f, m.as_slice()).await?;
    Ok(())
}

async fn read_exact(f: &mut Box<dyn File>, mut buf: &mut [u8]) -> Result<(), Error> {
    while !buf.is_empty() {
        let n = f.read(buf).await.expect("Failed to read");
        if n == 0 {
            return Err(Error::Eof);
        }
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
    f_read_half: Box<dyn File>,
    f_write_half: Box<dyn File>,
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
                write_message_to_f(&mut self.f_write_half, &response).await?;
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
        read_exact(&mut self.f_read_half, buf).await
    }
}
impl AblatedServer {
    pub fn new(
        f_read_half: Box<dyn File>,
        f_write_half: Box<dyn File>,
        shared_ns: Arc<Namespace>,
    ) -> Self {
        AblatedServer {
            f_read_half,
            f_write_half,
            shared_ns,
        }
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
    f_write_half: Box<dyn File>,
    file_request_future_recv: channel::Receiver<Option<String>>,
}

pub struct AblatedClientRx {
    f_read_half: Box<dyn File>,
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
    fn new(
        f_write_half: Box<dyn File>,
        file_request_future_recv: channel::Receiver<Option<String>>,
    ) -> Self {
        Self {
            f_write_half,
            file_request_future_recv,
        }
    }

    async fn close(&mut self) {
        let _ = write_message_to_f(&mut self.f_write_half, &Message::ClientClose).await;
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
                    let _ = write_message_to_f(&mut self.f_write_half, &m).await;
                }
            }
        }
    }
}

impl AblatedClientRx {
    fn new(
        f_read_half: Box<dyn File>,
        future_recv: channel::Receiver<Option<oneshot::Sender<Vec<u8>>>>,
    ) -> Self {
        Self {
            f_read_half,
            future_recv,
        }
    }

    pub async fn run(mut self) -> Result<(), Error> {
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
                        {
                            let mut readbuf = buf.as_mut_slice();
                            while !readbuf.is_empty() {
                                let n = self
                                    .f_read_half
                                    .read(readbuf)
                                    .await
                                    .expect("Failed to read msg size");
                                readbuf = &mut readbuf[n..];
                            }
                        }
                        let len = usize::from_ne_bytes(buf);

                        let mut message_buf = vec![0u8; len];
                        {
                            let mut message_readbuf = message_buf.as_mut_slice();
                            while !message_readbuf.is_empty() {
                                let n = self
                                    .f_read_half
                                    .read(message_readbuf)
                                    .await
                                    .expect("Failed to read content");
                                message_readbuf = &mut message_readbuf[n..];
                            }
                        }
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
    f_read_half: Box<dyn File>,
    f_write_half: Box<dyn File>,
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
        read_exact(&mut self.f_read_half, buf).await
    }
}

impl Drop for ContinuationServer {
    fn drop(&mut self) {
        let _ = self.continuations.send_blocking(None);
    }
}

impl ContinuationServer {
    pub fn new(
        f_read_half: Box<dyn File>,
        f_write_half: Box<dyn File>,
        continuations: channel::Sender<Option<Vec<u8>>>,
    ) -> ContinuationServer {
        ContinuationServer {
            f_read_half,
            f_write_half,
            continuations,
        }
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
        let _ = self.continuation_request_future_send.send_blocking(Some(data));
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
    f_write_half: Box<dyn File>,
    continuation_request_future_recv: channel::Receiver<Option<Vec<u8>>>,
}

impl ContinuationClientRelay {
    fn new(
        f_write_half: Box<dyn File>,
        continuation_request_future_recv: channel::Receiver<Option<Vec<u8>>>,
    ) -> Self {
        Self {
            f_write_half,
            continuation_request_future_recv,
        }
    }

    async fn close(&mut self) {
        let _ = write_message_to_f(&mut self.f_write_half, &Message::ClientClose).await;
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
                    let _ = write_message_to_f(&mut self.f_write_half, &m).await;
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

fn make_ablated_client(
    client_conn_read_end: Box<dyn File>,
    client_conn_write_end: Box<dyn File>,
) -> (Arc<AblatedClientTx>, AblatedClientRelay, AblatedClientRx) {
    let (file_request_future_send, file_request_future_recv) = channel::unbounded();
    let (future_send, future_recv) = channel::unbounded();

    (
        AblatedClientTx::new(file_request_future_send, future_send),
        AblatedClientRelay::new(client_conn_write_end, file_request_future_recv),
        AblatedClientRx::new(client_conn_read_end, future_recv),
    )
}

fn make_non_ablated_client(
    _client_conn_read_end: Box<dyn File>,
    client_conn_write_end: Box<dyn File>,
) -> (
    Arc<ContinuationClientTx>,
    ContinuationClientRelay,
    ContinuationClientRx,
) {
    let (future_send, future_recv) = channel::unbounded();

    (
        ContinuationClientTx::new(future_send),
        ContinuationClientRelay::new(client_conn_write_end, future_recv),
        ContinuationClientRx::new(),
    )
}

pub fn make_client(
    client_conn_read_end: Box<dyn File>,
    client_conn_write_end: Box<dyn File>,
) -> (Arc<ClientTx>, ClientRelay, ClientRx) {
    #[cfg(feature = "ablation")]
    let res = make_ablated_client(client_conn_read_end, client_conn_write_end);

    #[cfg(not(feature = "ablation"))]
    let res = make_non_ablated_client(client_conn_read_end, client_conn_write_end);

    res
}
