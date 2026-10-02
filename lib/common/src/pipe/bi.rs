use super::doorbell::DoorBell;
use super::error::Result;
use super::uni::{Reader, Writer, channel};

#[derive(Debug)]
pub struct Pipe<D: DoorBell> {
    rx: Option<Reader>,
    tx: Option<Writer>,
    rx_avail: D,
    tx_avail: D,
}

pub fn pipe<D0: DoorBell, D1: DoorBell>(
    len: usize,
    rx_avail0: D0,
    tx_avail0: D0,
    rx_avail1: D1,
    tx_avail1: D1,
) -> (Pipe<D0>, Pipe<D1>) {
    let (r0, w0) = channel(len);
    let (r1, w1) = channel(len);
    (
        Pipe {
            rx: Some(r0),
            tx: Some(w1),
            rx_avail: rx_avail0,
            tx_avail: tx_avail0,
        },
        Pipe {
            rx: Some(r1),
            tx: Some(w0),
            rx_avail: rx_avail1,
            tx_avail: tx_avail1,
        },
    )
}

impl<D: DoorBell> Pipe<D> {
    pub fn read(&mut self, data: &mut [u8]) -> Result<usize> {
        let res = self.rx.as_mut().unwrap().read(data);
        if let Ok(s) = res
            && s > 0
        {
            self.tx_avail.ring();
        }
        res
    }

    /// Whether a read can complete, including reporting a peer hangup.
    pub fn can_read(&self) -> bool {
        !self.rx.as_ref().unwrap().is_empty() || self.rx.as_ref().unwrap().is_closed()
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        let res = self.tx.as_mut().unwrap().write(data);
        if let Ok(s) = res
            && s > 0
        {
            self.rx_avail.ring();
        }
        res
    }

    /// Whether a write can complete, including reporting a peer hangup.
    pub fn can_write(&self) -> bool {
        !self.tx.as_ref().unwrap().is_empty() || self.tx.as_ref().unwrap().is_closed()
    }

    pub fn into_inner(mut self) -> (Reader, Writer, D, D) {
        // Suppress hangup notifications while transferring ownership of the pipe.
        let rx = self.rx.take().unwrap();
        let tx = self.tx.take().unwrap();
        let this = core::mem::ManuallyDrop::new(self);
        // SAFETY: this is never dropped and each doorbell is moved exactly once.
        unsafe {
            (
                rx,
                tx,
                core::ptr::read(&this.rx_avail),
                core::ptr::read(&this.tx_avail),
            )
        }
    }

    /// # Safety
    /// The reader and writer must correspond to the two halves of a pipe, as previously returned
    /// from into_inner.
    pub unsafe fn from_inner(rx: Reader, tx: Writer, rx_avail: D, tx_avail: D) -> Self {
        Pipe {
            rx: Some(rx),
            tx: Some(tx),
            rx_avail,
            tx_avail,
        }
    }
}

impl<D: DoorBell> Drop for Pipe<D> {
    fn drop(&mut self) {
        // Publish hangups before notifying the peer, just as for cursor updates.
        if self.rx.take().is_some() {
            self.tx_avail.ring();
        }
        if self.tx.take().is_some() {
            self.rx_avail.ring();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};

    pub struct TestDoorBell {
        count: AtomicUsize,
    }

    impl DoorBell for TestDoorBell {
        fn ring(&self) {
            self.count.fetch_add(1, Ordering::Release);
        }
    }

    impl TestDoorBell {
        pub fn new() -> Self {
            Self {
                count: AtomicUsize::new(0),
            }
        }
    }

    #[test]
    fn readiness_and_hangup_notifications() {
        use super::super::Error;
        use std::sync::Arc;

        struct Bell(Arc<AtomicUsize>);
        impl DoorBell for Bell {
            fn ring(&self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let readable = Arc::new(AtomicUsize::new(0));
        let writable = Arc::new(AtomicUsize::new(0));
        let (mut p, mut q) = pipe(
            4,
            Bell(readable.clone()),
            Bell(writable.clone()),
            Bell(Arc::new(AtomicUsize::new(0))),
            Bell(Arc::new(AtomicUsize::new(0))),
        );
        let (rx, tx, rx_avail, tx_avail) = p.into_inner();
        assert_eq!(readable.load(Ordering::SeqCst), 0);
        assert_eq!(writable.load(Ordering::SeqCst), 0);
        // Ownership transfer must preserve the open pipe without ringing hangups.
        p = unsafe { Pipe::from_inner(rx, tx, rx_avail, tx_avail) };
        assert!(!q.can_read());
        assert!(p.can_write());
        assert_eq!(p.write(b"abc"), Ok(3));
        assert!(!p.can_write());
        assert_eq!(readable.load(Ordering::SeqCst), 1);
        let mut buf = [0; 3];
        assert_eq!(q.read(&mut buf[..1]), Ok(1));
        assert!(p.can_write());
        drop(p);
        assert_eq!(readable.load(Ordering::SeqCst), 2);
        assert_eq!(writable.load(Ordering::SeqCst), 1);
        assert!(q.can_read());
        assert_eq!(q.read(&mut buf), Ok(2));
        assert_eq!(&buf[..2], b"bc");
        assert!(q.can_read()); // hangup must be observed without waiting
        assert_eq!(q.read(&mut buf), Err(Error::Closed));
        assert!(q.can_write());
        assert_eq!(q.write(b"x"), Err(Error::Closed));
    }

    #[test]
    pub fn test_ping_pong() {
        let (mut p, mut q) = super::pipe(
            1024,
            TestDoorBell::new(),
            TestDoorBell::new(),
            TestDoorBell::new(),
            TestDoorBell::new(),
        );
        std::thread::spawn(move || {
            loop {
                let mut buf = [0; 8];
                loop {
                    let result = q.read(&mut buf);
                    if result.is_ok() {
                        break;
                    }
                    std::thread::yield_now();
                }
                let i = u64::from_le_bytes(buf);
                buf = u64::to_le_bytes(i + 1);
                let _ = q.write(&buf);
            }
        });
        let mut bytes = u64::to_le_bytes(0);
        let mut i = 0;
        loop {
            p.write(&bytes).unwrap();
            loop {
                let result = p.read(&mut bytes);
                if result.is_ok() {
                    break;
                }
                std::thread::yield_now();
            }
            let j = u64::from_le_bytes(bytes);
            assert_eq!(j, i + 1);
            i = j + 1;
            bytes = u64::to_le_bytes(j + 1);
            if i >= 1024 {
                return;
            }
        }
    }
}
