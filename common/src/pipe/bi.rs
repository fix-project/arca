use super::error::Result;
use super::uni::{channel, Reader, Writer};

#[derive(Debug)]
pub struct Pipe {
    rx: Reader,
    tx: Writer,
}

pub fn pipe(len: usize) -> (Pipe, Pipe) {
    let (r0, w0) = channel(len);
    let (r1, w1) = channel(len);
    (Pipe { rx: r0, tx: w1 }, Pipe { rx: r1, tx: w0 })
}

impl Pipe {
    pub fn read(&mut self, data: &mut [u8]) -> Result<usize> {
        self.rx.read(data)
    }

    pub fn can_read(&self) -> bool {
        !self.rx.is_empty()
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        self.tx.write(data)
    }

    pub fn can_write(&self) -> bool {
        !self.tx.is_empty()
    }

    pub fn into_inner(self) -> (Reader, Writer) {
        (self.rx, self.tx)
    }

    /// # Safety
    /// The reader and writer must correspond to the two halves of a pipe, as previously returned
    /// from into_inner.
    pub unsafe fn from_inner(rx: Reader, tx: Writer) -> Self {
        Pipe { rx, tx }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    pub fn test_ping_pong() {
        let (mut p, mut q) = super::pipe(1024);
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
