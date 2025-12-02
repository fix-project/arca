use core::panic;

use alloc::collections::vec_deque::VecDeque;
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

async fn write_message_to_f(
    f: &Arc<SpinLock<Box<dyn File>>>,
    message: &Message,
) -> Result<(), Error> {
    let m = postcard::to_allocvec(message).unwrap();
    let buf = m.len().to_ne_bytes();
    let mut f = f.lock();
    write_all(&mut f, &buf).await?;
    write_all(&mut f, m.as_slice()).await?;
    Ok(())
}

#[async_trait]
pub trait MessageHandler {
    async fn process_message(&self, msg: Message) -> Result<(), Error>;
}

pub struct AblatedHandler {
    f: Arc<SpinLock<Box<dyn File>>>,
    shared_ns: Arc<Namespace>,
    futures: SpinLock<VecDeque<oneshot::Sender<Vec<u8>>>>,
}

#[async_trait]
impl MessageHandler for AblatedHandler {
    async fn process_message(&self, msg: Message) -> Result<(), Error> {
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

                let response = Message::FileResponse(FileResponse { file_data });
                write_message_to_f(&self.f, &response).await?;
                Ok(())
            }
            Message::FileResponse(FileResponse { file_data }) => {
                log::info!("Received File Response");

                self.futures
                    .lock()
                    .pop_front()
                    .expect("Received File Response without matching File Request")
                    .send(file_data);

                Ok(())
            }
            Message::Continuation(_) => {
                panic!("Should not receive continuation message in ablated handler")
            }
        }
    }
}

impl AblatedHandler {
    pub fn new(f: Arc<SpinLock<Box<dyn File>>>, shared_ns: Arc<Namespace>) -> Arc<AblatedHandler> {
        Arc::new(AblatedHandler {
            f,
            shared_ns,
            futures: SpinLock::new(VecDeque::new()),
        })
    }

    pub async fn request_file(
        &self,
        file_path: String,
    ) -> Result<oneshot::Receiver<Vec<u8>>, Error> {
        let m = Message::FileRequest(FileRequest { file_path });
        let (sender, receiver) = oneshot::channel();
        {
            let mut guard = self.futures.lock();
            guard.push_back(sender);
            write_message_to_f(&self.f, &m).await?;
        }
        Ok(receiver)
    }
}

pub struct ContinuationHandler {
    f: Arc<SpinLock<Box<dyn File>>>,
    continuations: Arc<SpinLock<VecDeque<Vec<u8>>>>,
}

#[async_trait]
impl MessageHandler for ContinuationHandler {
    async fn process_message(&self, msg: Message) -> Result<(), Error> {
        match msg {
            Message::Continuation(Continuation { data }) => {
                self.continuations.lock().push_back(data);
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
}

impl ContinuationHandler {
    pub fn new(
        f: Arc<SpinLock<Box<dyn File>>>,
        continuations: Arc<SpinLock<VecDeque<Vec<u8>>>>,
    ) -> Arc<ContinuationHandler> {
        Arc::new(ContinuationHandler { f, continuations })
    }

    pub async fn request_to_run(&self, data: Vec<u8>) -> Result<(), Error> {
        let m = Message::Continuation(Continuation { data });
        write_message_to_f(&self.f, &m).await?;
        Ok(())
    }
}

pub struct TcpHandler<H: MessageHandler + Send> {
    f: Arc<SpinLock<Box<dyn File>>>,
    handler: Arc<H>,
}

impl<H: MessageHandler + Send> TcpHandler<H> {
    pub fn new(f: Arc<SpinLock<Box<dyn File>>>, handler: Arc<H>) -> Self {
        Self { f, handler }
    }

    pub async fn run(self) -> Result<(), Error> {
        loop {
            let msg = {
                let mut buf = [0u8; 8];
                self.f
                    .lock()
                    .read_exact(&mut buf)
                    .await
                    .map_err(|_| Error::VfsError)?;
                let len = usize::from_ne_bytes(buf);

                let mut message_buf = vec![0u8; len];
                self.f
                    .lock()
                    .read_exact(message_buf.as_mut_slice())
                    .await
                    .map_err(|_| Error::VfsError)?;

                postcard::from_bytes(message_buf.as_slice()).unwrap()
            };
            self.handler.process_message(msg).await?;
        }
    }
}
