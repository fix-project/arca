use crate::Client;
use std::net::{SocketAddr, ToSocketAddrs};

pub(crate) fn run(client: &mut Client, listen: &str) -> Result<(), Box<dyn std::error::Error>> {
    let address = listen
        .to_socket_addrs()?
        .find_map(|address| match address {
            SocketAddr::V4(address) => Some(address),
            SocketAddr::V6(_) => None,
        })
        .ok_or("no IPv4 listen address")?;
    let collector = client.lookup(&client.options().collector.clone())?;
    let memory = client.blob(&((client.options().memory_mib as u64) << 20).to_le_bytes())?;
    let ip = client.blob(&address.ip().octets())?;
    let port = client.blob(&u64::from(address.port()).to_le_bytes())?;
    let input = client.tree(&[ip, port, memory])?;
    let scheduler = client.lookup(&client.options().scheduler.clone())?;
    let mode = client.blob(b"serve")?;
    let limits = client.blob(b"")?;
    let combination = client.tree(&[limits, scheduler, mode, collector, input])?;
    client.schedule(&combination)?;
    Ok(())
}
