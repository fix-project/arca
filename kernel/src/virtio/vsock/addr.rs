use core::str::FromStr;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct SocketAddr {
    pub cid: u64,
    pub port: u32,
}

impl SocketAddr {
    pub fn new(cid: u64, port: u32) -> SocketAddr {
        Self { cid, port }
    }
}

pub struct InvalidSockAddr;

impl FromStr for SocketAddr {
    type Err = InvalidSockAddr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (cid, port) = s.split_once(":").ok_or(InvalidSockAddr)?;
        let cid: u64 = str::parse(cid).map_err(|_| InvalidSockAddr)?;
        let port: u32 = str::parse(port).map_err(|_| InvalidSockAddr)?;
        Ok(SocketAddr { cid, port })
    }
}

impl TryFrom<&str> for SocketAddr {
    type Error = InvalidSockAddr;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        SocketAddr::from_str(value)
    }
}
