use std::{
    io::{Read, Write},
    net::TcpStream,
};

use vsock::VsockStream;

fn copy(src: &mut dyn Read, dst: &mut dyn Write) -> std::io::Result<()> {
    let mut buf = [0_u8; 16 * 1024];

    loop {
        let n = match src.read(&mut buf) {
            Ok(0) => {
                let _ = dst.flush();
                return Ok(());
            }
            Ok(n) => n,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                } else {
                    return Err(e);
                }
            }
        };

        let mut written = 0;

        while written < n {
            written += match dst.write(&buf[written..n]) {
                Ok(0) => {
                    // TODO; unexpected hang up
                    return Ok(());
                }
                Ok(n) => n,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    } else {
                        return Err(e);
                    }
                }
            };
        }
        log::debug!("Forwarding {written}");
    }
}

pub async fn relay_tcp_vsock(tcp: TcpStream, vsock: VsockStream) -> std::io::Result<()> {
    let mut tcp_write_half = tcp.try_clone()?;
    let mut tcp_read_half = tcp;

    let mut vsock_write_half = vsock.try_clone()?;
    let mut vsock_read_half = vsock;

    let j1 = smol::unblock(move || copy(&mut tcp_read_half, &mut vsock_write_half));
    let j2 = smol::unblock(move || copy(&mut vsock_read_half, &mut tcp_write_half));

    let _ = j1.await;
    let _ = j2.await;

    Ok(())
}
