use super::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Flow {
    pub src: SocketAddr,
    pub dst: SocketAddr,
}

impl Flow {
    pub fn reverse(&self) -> Flow {
        Flow {
            dst: self.src,
            src: self.dst,
        }
    }
}
