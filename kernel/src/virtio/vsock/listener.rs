use core::{
    future::Future,
    task::{Poll, Waker},
};

use alloc::collections::vec_deque::VecDeque;

use crate::rt;

use super::*;

pub(crate) struct ListenSocket {
    pub addr: SocketAddr,
    pub waker: Option<Waker>,
    pub queue: VecDeque<Flow>,
}

pub struct StreamListener {
    socket: Arc<RwLock<ListenSocket>>,
    pub addr: SocketAddr,
}

struct Accept {
    socket: Arc<RwLock<ListenSocket>>,
}

impl StreamListener {
    #[rt::profile]
    pub async fn bind(port: u32) -> Result<StreamListener> {
        let socket = listen(SocketAddr { cid: 3, port }).await;
        let addr = socket.read().addr;
        Ok(StreamListener { socket, addr })
    }

    #[rt::profile]
    pub async fn accept(&self) -> Result<Stream> {
        let flow = Accept {
            socket: self.socket.clone(),
        }
        .await;
        let socket = accept(flow).await;
        Ok(Stream::new(socket))
    }
}

impl Future for Accept {
    type Output = Flow;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let mut socket = self.socket.write();
        if let Some(x) = socket.queue.pop_front() {
            Poll::Ready(x)
        } else {
            socket.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
