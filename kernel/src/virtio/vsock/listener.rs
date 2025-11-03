use core::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

use alloc::collections::vec_deque::VecDeque;

use crate::rt;

use super::*;

pub(crate) struct ListenSocket {
    pub addr: SocketAddr,
    pub waker: Option<Waker>,
    pub queue: VecDeque<Flow>,
}

impl Drop for ListenSocket {
    fn drop(&mut self) {
        panic!("who dropped a ListenSocket?");
    }
}

pub struct StreamListener {
    socket: Arc<RwLock<ListenSocket>>,
    pub addr: SocketAddr,
}

impl Drop for StreamListener {
    fn drop(&mut self) {
        panic!("who dropped a StreamListener?");
    }
}

struct Accept {
    socket: Arc<RwLock<ListenSocket>>,
}

impl StreamListener {
    #[rt::profile]
    pub async fn bind(port: u32) -> Result<StreamListener> {
        let socket = listen(SocketAddr { cid: 3, port }).await;
        let addr = socket.read().await.addr;
        Ok(StreamListener { socket, addr })
    }

    #[rt::profile]
    pub async fn accept(&self) -> Result<Stream> {
        let flow = Accept {
            socket: self.socket.clone(),
        }
        .await;
        let socket = accept(flow).await;
        Ok(Stream::new(socket).await)
    }
}

impl Future for Accept {
    type Output = Flow;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let mut fake_cx = Context::from_waker(Waker::noop());
        let mut socket = loop {
            let socket = pin! {self.socket.write()};
            if let Poll::Ready(result) = Future::poll(socket, &mut fake_cx) {
                break result;
            }
            core::hint::spin_loop();
        };
        if let Some(x) = socket.queue.pop_front() {
            Poll::Ready(x)
        } else {
            socket.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
