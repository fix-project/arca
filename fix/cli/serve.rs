use crate::Client;
use std::net::{SocketAddr, ToSocketAddrs};

fn restart(client: &Client, scheduler: [u8; 32], collector: [u8; 32]) -> std::io::Error {
    use std::os::unix::process::CommandExt;
    let empty = *fixhandle::Blob::literal(b"")
        .unwrap()
        .as_handle()
        .as_bytes();
    let replaced = |argument: &str| {
        (scheduler != empty && argument == "--scheduler")
            || (collector != empty && argument == "--collector")
    };
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        let text = argument.to_string_lossy();
        if text == "-C" || replaced(&text) {
            arguments.next();
        } else if !text
            .split_once('=')
            .is_some_and(|(option, _)| replaced(option))
        {
            command.arg(argument);
        }
    }
    command.arg("-C").arg(&client.directory);
    for (option, name) in [("--scheduler", scheduler), ("--collector", collector)] {
        if name != empty {
            command.args([option, &hex::encode(name)]);
        }
    }
    command.env("FIX_SERVE_ACK", hex::encode(scheduler)).exec()
}

pub(crate) fn run(
    client: &mut Client,
    listen: Option<&str>,
    stdio: bool,
) -> Result<(), Box<dyn std::error::Error>> {
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
    let limits = client.blob(b"")?;
    let combination = client.tree(&[limits, scheduler, mode, collector, input])?;
    if stdio {
        if let Some(name) = std::env::var_os("FIX_SERVE_ACK") {
            use std::io::Write;
            let mut bytes = [0; 32];
            hex::decode_to_slice(name.to_string_lossy().as_bytes(), &mut bytes)
                .map_err(|error| format!("invalid server restart acknowledgement: {error}"))?;
            unsafe { std::env::remove_var("FIX_SERVE_ACK") };
            let message =
                fixremote::protocol::encode(fixremote::protocol::Message::ObjectReceived(bytes))?;
            let mut output = std::io::stdout().lock();
            output.write_all(&(message.len() as u64).to_be_bytes())?;
            output.write_all(&message)?;
            output.flush()?;
        }
        let result = client.schedule_stream(&combination)?;
        if client.len(&result)? == 2 {
            let scheduler = client.get(&result, 0)?;
            let collector = client.get(&result, 1)?;
            return Err(restart(
                client,
                *client.handle(&scheduler)?.as_bytes(),
                *client.handle(&collector)?.as_bytes(),
            )
            .into());
        }
    } else {
        client.schedule(&combination)?;
    }
    Ok(())
}
