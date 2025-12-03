use core::panic;

use async_trait::async_trait;
use kernel::prelude::*;
use serde::{Deserialize, Serialize};
use vfs::{File, FileExt, Open};

use crate::proc::Namespace;

#[derive(Debug)]
pub enum Error {
    VfsError,
    MessageProcessingError,
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
}

async fn write_all(f: &mut Box<dyn File>, mut buf: &[u8]) -> Result<(), Error> {
    while !buf.is_empty() {
        match f.write(buf).await {
            Ok(0) => return Err(Error::VfsError),
            Ok(n) => {
                buf = &buf[n..];
            }
            Err(_) => {
                return Err(Error::VfsError);
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

#[async_trait]
pub trait MessageServer {
    async fn process_message(&mut self, msg: Message) -> Result<(), Error>;
    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error>;
}

pub struct AblatedServer {
    f: Box<dyn File>,
    shared_ns: Arc<Namespace>,
}

#[async_trait]
impl MessageServer for AblatedServer {
    async fn process_message(&mut self, msg: Message) -> Result<(), Error> {
        match msg {
            Message::FileRequest(FileRequest { file_path }) => {
                log::info!("Received File Request for path {}", file_path);
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
                let mut buffer = [0u8; 4096];
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

                log::info!("Read data {}", file_path);

                let response = Message::FileResponse(FileResponse { file_data });
                write_message_to_f(&mut self.f, &response).await?;
                log::info!("Replied {}", file_path);
                Ok(())
            }
            Message::FileResponse(_) => {
                panic!("FileResponse should be handled AblatedClient")
            }
            Message::Continuation(_) => {
                panic!("Should not receive continuation message in ablated handler")
            }
        }
    }

    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        self.f.read_exact(buf).await.map_err(|_| Error::VfsError)
    }
}
impl AblatedServer {
    pub fn new(f: Box<dyn File>, shared_ns: Arc<Namespace>) -> Self {
        AblatedServer { f, shared_ns }
    }
}

pub struct AblatedClient {
    f: SpinLock<Box<dyn File>>,
    future_send: channel::Sender<oneshot::Sender<Vec<u8>>>,
    future_recv: channel::Receiver<oneshot::Sender<Vec<u8>>>,
}

impl AblatedClient {
    pub fn new(f: Box<dyn File>) -> Arc<Self> {
        let (future_send, future_recv) = channel::unbounded();
        Arc::new(Self {
            f: SpinLock::new(f),
            future_send,
            future_recv,
        })
    }

    pub async fn request_file(
        &self,
        file_path: String,
    ) -> Result<oneshot::Receiver<Vec<u8>>, Error> {
        let m = Message::FileRequest(FileRequest { file_path });
        let (sender, receiver) = oneshot::channel();
        {
            log::info!("Try to lock on self.futures");
            let mut guard = self.f.lock();
            write_message_to_f(&mut guard, &m).await?;
            self.future_send
                .send_blocking(sender)
                .expect("Failed to send blocking");
        }
        Ok(receiver)
    }

    pub async fn run(self: Arc<Self>) -> Result<(), Error> {
        loop {
            let x = self.future_recv.recv().await;

            match x {
                Err(_) => return Ok(()),
                Ok(future) => {
                    let msg: Message = {
                        let mut buf = [0u8; 8];
                        {
                            let mut readbuf = buf.as_mut_slice();
                            while !readbuf.is_empty() {
                                let n = self
                                    .f
                                    .lock()
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
                                    .f
                                    .lock()
                                    .read(message_readbuf)
                                    .await
                                    .expect("Failed to read content");
                                message_readbuf = &mut message_readbuf[n..];
                            }
                        }
                        log::info!("Read msg content");

                        postcard::from_bytes(message_buf.as_slice()).unwrap()
                    };

                    match msg {
                        Message::FileResponse(FileResponse { file_data }) => {
                            log::info!("Received File Response");
                            future.send(file_data);
                        }
                        Message::FileRequest(_) => panic!(),
                        Message::Continuation(_) => panic!(),
                    }
                }
            }
        }
    }
}

pub struct ContinuationServer {
    f: Box<dyn File>,
    continuations: channel::Sender<Vec<u8>>,
}

#[async_trait]
impl MessageServer for ContinuationServer {
    async fn process_message(&mut self, msg: Message) -> Result<(), Error> {
        match msg {
            Message::Continuation(Continuation { data }) => {
                self.continuations
                    .send_blocking(data)
                    .expect("Failed to send continuation data");
                Ok(())
            }
            Message::FileResponse(_) => {
                panic!("Should not receive File Response in ContinuationHandler")
            }
            Message::FileRequest(_) => {
                panic!("Should not receive File Request in ContinuationHandler")
            }
        }
    }

    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        self.f.read_exact(buf).await.map_err(|_| Error::VfsError)
    }
}

impl ContinuationServer {
    pub fn new(f: Box<dyn File>, continuations: channel::Sender<Vec<u8>>) -> ContinuationServer {
        ContinuationServer { f, continuations }
    }
}

pub struct ContinuationClient {
    f: SpinLock<Box<dyn File>>,
}

impl ContinuationClient {
    pub fn new(f: Box<dyn File>) -> Arc<Self> {
        Arc::new(Self {
            f: SpinLock::new(f),
        })
    }

    pub async fn request_to_run(&self, data: Vec<u8>) -> Result<(), Error> {
        let m = Message::Continuation(Continuation { data });
        write_message_to_f(&mut self.f.lock(), &m).await?;
        Ok(())
    }

    pub async fn run(self: Arc<Self>) -> Result<(), Error> {
        Ok(())
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
                log::info!("Locking to read size");
                let mut buf = [0u8; 8];
                self.handler.read_exact(&mut buf).await?;
                let len = usize::from_ne_bytes(buf);

                log::info!("Locking to read msg content");
                let mut message_buf = vec![0u8; len];
                self.handler.read_exact(&mut message_buf).await?;
                log::info!("Read msg content");

                postcard::from_bytes(message_buf.as_slice()).unwrap()
            };
            self.handler.process_message(msg).await?;
        }
    }
}

#[cfg(feature = "ablation")]
pub type Client = AblatedClient;
#[cfg(not(feature = "ablation"))]
pub type Client = ContinuationClient;
