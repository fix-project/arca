#![allow(unused)]

use common::message::frame_codec::Error as FrameCodecError;
use common::message::frame_codec::Frame;
use common::message::frame_codec::FrameReadBuf;
use common::message::frame_codec::FrameWriteBuf;
use common::message::traits::Error as MessageCodecError;
use common::message::traits::IntoBytes;
use common::pipe::OwnedSplit;
use common::pipe::Read;
use common::pipe::Write;
use std::io::Error as IOError;
use std::marker::PhantomData;
use tokio::sync::mpsc;
use vmm_sys_util::eventfd::EventFd;

use futures::Future;
use tokio::{io::unix::AsyncFd, task::JoinHandle};

use crate::doorbell::VMToHostDoorBellWaiter;

#[derive(Debug)]
pub enum Error {
    Codec(MessageCodecError),
    FrameCodec(FrameCodecError),
    IO(IOError),
    Channel,
    RequestHandling,
}

impl From<MessageCodecError> for Error {
    fn from(value: MessageCodecError) -> Self {
        Error::Codec(value)
    }
}

impl From<FrameCodecError> for Error {
    fn from(value: FrameCodecError) -> Self {
        Error::FrameCodec(value)
    }
}

impl From<IOError> for Error {
    fn from(value: IOError) -> Self {
        Error::IO(value)
    }
}

pub trait StatefulMonitor<const MAX_FRAME_PAYLOAD: usize> {
    type Request: TryFrom<Frame<MAX_FRAME_PAYLOAD>, Error = MessageCodecError> + Send;
    type Reply: IntoBytes + Send;

    fn handle_request(
        &mut self,
        req: Self::Request,
    ) -> impl Future<Output = Result<Self::Reply, Error>> + Send;
}

struct Monitor<T, P, const MAX_FRAME_PAYLOAD: usize>
where
    T: StatefulMonitor<MAX_FRAME_PAYLOAD> + Send + 'static,
    P: OwnedSplit + 'static,
{
    handles: Vec<JoinHandle<Result<(), Error>>>,
    _marker: std::marker::PhantomData<T>,
    _p_marker: std::marker::PhantomData<P>,
}

impl<
        T: StatefulMonitor<MAX_FRAME_PAYLOAD> + Send + 'static,
        P: OwnedSplit + 'static,
        const MAX_FRAME_PAYLOAD: usize,
    > Monitor<T, P, MAX_FRAME_PAYLOAD>
{
    async fn wait_and_drain(fd: &AsyncFd<EventFd>) -> Result<(), Error> {
        let mut guard = fd.readable().await?;

        // Drain event fd
        loop {
            match guard.try_io(|inner| inner.get_ref().read()) {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => return Err(e.into()),
                Err(_would_block) => break,
            };
        }

        Ok(())
    }

    async fn handle_request(
        mut state: T,
        mut request_rx: mpsc::UnboundedReceiver<T::Request>,
        reply_tx: mpsc::UnboundedSender<T::Reply>,
    ) -> Result<(), Error> {
        loop {
            let request = request_rx.recv().await;

            if request.is_none() {
                return Ok(());
            }

            let reply = state.handle_request(request.unwrap()).await?;
            reply_tx.send(reply).map_err(|_| Error::Channel)?;
        }
    }

    async fn pipe_to_request(
        mut pipe_rx: impl Read,
        read_bell_waiter: AsyncFd<EventFd>,
        request_tx: mpsc::UnboundedSender<T::Request>,
    ) -> Result<(), Error> {
        let mut read_frame_buf = FrameReadBuf::<MAX_FRAME_PAYLOAD>::default();

        loop {
            Self::wait_and_drain(&read_bell_waiter).await?;

            // Read from pipe to read_frame_buf
            while let Ok(res) = read_frame_buf.try_read_frame(&mut pipe_rx) {
                if let Some(frame) = res {
                    let req = T::Request::try_from(frame).unwrap();
                    request_tx.send(req).map_err(|_| Error::Channel)?;
                }
            }
        }
    }

    async fn reply_to_pipe(
        mut pipe_tx: impl Write,
        write_bell_waiter: AsyncFd<EventFd>,
        mut reply_rx: mpsc::UnboundedReceiver<T::Reply>,
    ) -> Result<(), Error> {
        let mut write_frame_buf = FrameWriteBuf::default();

        loop {
            // Block on new reply
            let reply = reply_rx.recv().await;

            if reply.is_none() {
                return Ok(());
            }

            let reply = reply.unwrap();
            let frame = reply.into_boxed_slice();

            write_frame_buf
                .load(frame)
                .map_err(<common::message::frame_codec::Error as Into<Error>>::into)?;

            // Write as much as possible
            loop {
                match write_frame_buf.try_write_frame(&mut pipe_tx) {
                    Ok(true) => break,
                    Ok(false) => {
                        // TODO wait on event_fd
                        Self::wait_and_drain(&write_bell_waiter).await?;
                    }
                    Err(e) => return Err(Error::FrameCodec(e)),
                }
            }
        }
    }

    fn run(
        state: T,
        pipe: P,
        read_bell_waiter: VMToHostDoorBellWaiter,
        write_bell_waiter: VMToHostDoorBellWaiter,
    ) -> Result<Self, Error> {
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (reply_tx, reply_rx) = mpsc::unbounded_channel();
        let (pipe_rx, pipe_tx) = pipe.split();

        let handles = vec![
            (tokio::spawn(Self::pipe_to_request(
                pipe_rx,
                AsyncFd::new(read_bell_waiter.into())?,
                request_tx,
            ))),
            tokio::spawn(Self::handle_request(state, request_rx, reply_tx)),
            tokio::spawn(Self::reply_to_pipe(
                pipe_tx,
                AsyncFd::new(write_bell_waiter.into())?,
                reply_rx,
            )),
        ];

        Ok(Self {
            handles,
            _marker: PhantomData,
            _p_marker: PhantomData,
        })
    }
}
