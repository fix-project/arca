use core::fmt;
use core::str::FromStr;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct IpAddr {
    pub octets: [u8; 4],
    pub port: u16,
}

impl IpAddr {
    pub fn new(octets: [u8; 4], port: u16) -> IpAddr {
        Self { octets, port }
    }
}

pub struct InvalidAddr;

impl fmt::Debug for InvalidAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InvalidAddr")
    }
}

impl FromStr for IpAddr {
    type Err = InvalidAddr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (addr, port) = s.split_once(":").ok_or(InvalidAddr)?;

        let mut octets = [0u8; 4];
        let mut octets_index = 0;
        let mut remaining_addr = addr;
        // TODO(kmohr) I'm choosing to only support IPv4 for now
        while let Some((head, tail)) = remaining_addr.split_once('.') {
            if octets_index >= 4 {
                return Err(InvalidAddr);
            }
            let byte: u8 = str::parse(head).map_err(|_| InvalidAddr)?;
            octets[octets_index] = byte;
            octets_index += 1;
            remaining_addr = tail;
        }
        if octets_index != 3 {
            return Err(InvalidAddr);
        }

        let port: u16 = str::parse(port).map_err(|_| InvalidAddr)?;
        Ok(IpAddr { octets, port })
    }
}

impl fmt::Display for IpAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}:{}",
            self.octets[0], self.octets[1], self.octets[2], self.octets[3], self.port
        )
    }
}

impl TryFrom<&str> for IpAddr {
    type Error = InvalidAddr;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        IpAddr::from_str(value)
    }
}

impl From<u64> for IpAddr {
    fn from(addr: u64) -> Self {
        let port = ((addr >> 32) & 0xFFFF) as u16;
        let octet1 = ((addr >> 24) & 0xFF) as u8;
        let octet2 = ((addr >> 16) & 0xFF) as u8;
        let octet3 = ((addr >> 8) & 0xFF) as u8;
        let octet4 = (addr & 0xFF) as u8;
        IpAddr {
            octets: [octet1, octet2, octet3, octet4],
            port,
        }
    }
}

// Store this as u64 like
// |  0000 (16 bits) | port (16 bits) | octet1 (8 bits) | octet2 (8 bits) | octet3 (8 bits) | octet4 (8 bits) |
impl From<IpAddr> for u64 {
    fn from(addr: IpAddr) -> Self {
        ((addr.port as u64) << 32)
            | ((addr.octets[0] as u64) << 24)
            | ((addr.octets[1] as u64) << 16)
            | ((addr.octets[2] as u64) << 8)
            | (addr.octets[3] as u64)
    }
}
