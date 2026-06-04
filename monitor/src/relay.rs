use arca_pipe::{BidirectionalPipe, PipeError, Read, Write};
use std::io::{Read as IoRead, Write as IoWrite};
use std::net::{Shutdown, TcpStream};

/// Read from TCP and write into the pipe.
///
/// - If the pipe's peer read side is closed (Arca stopped reading), closes the
///   pipe write side and shuts down the TCP receive direction.
/// - On TCP EOF (`Ok(0)`), closes the pipe write side to signal Arca.
/// - On TCP error, closes the pipe write side and returns the error.
/// - Spins on pipe `WouldBlock` until all bytes from the TCP read are delivered.
pub fn tcp_to_pipe(tcp: &mut TcpStream, pipe: &mut BidirectionalPipe) -> std::io::Result<usize> {
    if pipe.is_peer_read_closed() {
        pipe.close_write();
        let _ = tcp.shutdown(Shutdown::Read);
        return Ok(0);
    }

    let mut buf = [0u8; 4096];
    match tcp.read(&mut buf) {
        Ok(0) => {
            pipe.close_write();
            Ok(0)
        }
        Ok(n) => {
            pipe.write_all(&buf[..n]);
            Ok(n)
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
        Err(e) => {
            pipe.close_write();
            Err(e)
        }
    }
}

/// Read from the pipe and write to TCP.
///
/// - On pipe `WouldBlock` with peer write closed (Arca done sending), closes
///   the pipe read side and shuts down the TCP send direction.
/// - On TCP write error, closes the pipe read side to signal Arca.
pub fn pipe_to_tcp(tcp: &mut TcpStream, pipe: &mut BidirectionalPipe) -> std::io::Result<usize> {
    let mut buf = [0u8; 4096];
    match pipe.read(&mut buf) {
        Ok(n) => {
            if let Err(e) = tcp.write_all(&buf[..n]) {
                pipe.close_read();
                return Err(e);
            }
            Ok(n)
        }
        Err(PipeError::WouldBlock) => {
            if pipe.is_peer_write_closed() {
                pipe.close_read();
                let _ = tcp.shutdown(Shutdown::Write);
            }
            Ok(0)
        }
    }
}
