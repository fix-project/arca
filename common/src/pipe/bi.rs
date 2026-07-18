use super::doorbell::DoorBell;
use super::error::Result;
use super::uni::{channel, Reader, Writer};

#[derive(Debug)]
pub struct Pipe<D: DoorBell> {
    rx: Reader,
    tx: Writer,
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
            rx: r0,
            tx: w1,
            rx_avail: rx_avail0,
            tx_avail: tx_avail0,
        },
        Pipe {
            rx: r1,
            tx: w0,
            rx_avail: rx_avail1,
            tx_avail: tx_avail1,
        },
    )
}

impl<D: DoorBell> Pipe<D> {
    pub fn read(&mut self, data: &mut [u8]) -> Result<usize> {
        let res = self.rx.read(data);
        if let Ok(s) = res {
            if s > 0 {
                self.tx_avail.ring();
            }
        }
        res
    }

    pub fn can_read(&self) -> bool {
        !self.rx.is_empty()
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        let res = self.tx.write(data);
        if let Ok(s) = res {
            if s > 0 {
                self.rx_avail.ring();
            }
        }
        res
    }

    pub fn can_write(&self) -> bool {
        !self.tx.is_empty()
    }

    pub fn into_inner(self) -> (Reader, Writer, D, D) {
        (self.rx, self.tx, self.rx_avail, self.tx_avail)
    }

    /// # Safety
    /// The reader and writer must correspond to the two halves of a pipe, as previously returned
    /// from into_inner.
    pub unsafe fn from_inner(rx: Reader, tx: Writer, rx_avail: D, tx_avail: D) -> Self {
        Pipe {
            rx,
            tx,
            rx_avail,
            tx_avail,
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
    pub fn test_ping_pong() {
        let (mut p, mut q) = super::pipe(
            1024,
            TestDoorBell::new(),
            TestDoorBell::new(),
            TestDoorBell::new(),
            TestDoorBell::new(),
        );
        std::thread::spawn(move || loop {
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
