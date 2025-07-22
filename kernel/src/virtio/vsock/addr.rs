#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct SocketAddr {
    pub cid: u64,
    pub port: u32,
}

impl SocketAddr {
    pub fn new(cid: u64, port: u32) -> SocketAddr {
        Self { cid, port }
    }
}
