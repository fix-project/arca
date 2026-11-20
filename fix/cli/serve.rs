use crate::Client;
use std::net::{SocketAddr, ToSocketAddrs};

fn prepare(client: &mut Client) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use fixremote::{linux::Duplex, protocol};
    let mut stream = Duplex {
        input: std::io::stdin(),
        output: std::io::stdout(),
    };
    let first = protocol::receive(&mut stream, protocol::MAX_MESSAGE_SIZE)
        .map_err(|error| error.to_string())?
        .ok_or("connection closed before its first request")?;
    let staging = matches!(&first, protocol::Message::ProposeTransfer { task: Some(task), .. } if task.is_stage_programs());
    if !staging {
        let bytes = protocol::encode(first)?;
        let mut framed = (bytes.len() as u64).to_be_bytes().to_vec();
        framed.extend(bytes);
        return Ok(framed);
    }
    let config = client.remote_config()?;
    let incoming =
        fixremote::transport::complete(protocol::asynchronous::receive_transfer_with_info_from(
            &mut fixremote::transport::Blocking(&mut stream),
            protocol::MAX_MESSAGE_SIZE,
            &protocol::Information::default(),
            &config.policy.authorized_keys,
            |offers| fixremote::missing(client.storage.repository(), offers).ok(),
            Some(first),
        ))
        .map_err(|error| error.to_string())?
        .ok_or("connection closed during program staging")?;
    let task = incoming.task.ok_or("missing program staging task")?;
    if incoming.result.is_some() {
        return Err("unexpected program staging result".into());
    }
    let roots = fixremote::import(
        client.storage.repository(),
        &incoming.transfer.encode()?,
        &config,
    )?;
    let [collector, scheduler] = roots.as_slice() else {
        return Err("expected a scheduler and collector".into());
    };
    if !collector.is_empty() {
        let mut trust = fixtrust::Trust::parse(&config.policy.trusted_compilers.join("\n"))
            .map_err(|error| error.to_string())?;
        for name in &config.policy.trusted_taggers {
            trust
                .trust_tagger_name(name)
                .map_err(|error| error.to_string())?;
        }
        if !trust
            .is_tagger(
                &*client.storage,
                fixhandle::Tree::try_from(*collector).map_err(|error| format!("{error:?}"))?,
            )
            .map_err(|error| error.to_string())?
        {
            return Err(
                "requested coupon collector is not a trusted tagger on this machine".into(),
            );
        }
        client.options.collector = hex::encode(collector.as_bytes());
    }
    if !scheduler.is_empty() {
        client.options.scheduler = hex::encode(scheduler.as_bytes());
    }
    client.ensure_session()?;
    protocol::send(&mut stream, protocol::Message::ObjectReceived(task.input))
        .map_err(|error| error.to_string())?;
    Ok(Vec::new())
}

pub(crate) fn run(
    client: &mut Client,
    listen: Option<&str>,
    stdio: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let prefix = if stdio { prepare(client)? } else { Vec::new() };
    client.ensure_session()?;
    let address = listen
        .unwrap_or("127.0.0.1:0")
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
    let mode = client.blob(&(stdio as u64).to_le_bytes())?;
    let input = client.tree(&[ip, port, memory, mode])?;
    let scheduler = client.lookup(&client.options().scheduler.clone())?;
    let mode = client.blob(b"serve")?;
    let limits = client.blob(&(1u64 << 30).to_le_bytes())?;
    let combination = client.tree(&[limits, scheduler, mode, collector, input])?;
    if stdio {
        client.schedule_stream_prefetched(&combination, prefix)?;
    } else {
        client.schedule(&combination)?;
    }
    Ok(())
}
