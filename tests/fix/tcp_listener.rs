#![no_std]
extern crate alloc;

use fixwasm::{
    Blob, Error, Handle, Object, Tree, Value,
    stream::{Stream, TcpListener},
};

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

fn send(stream: &Stream, mut bytes: &[u8]) -> Result<(), Error> {
    while !bytes.is_empty() {
        let sent = stream.send(bytes)?.expect("send failed");
        assert!(sent > 0);
        bytes = &bytes[sent..];
    }
    Ok(())
}

fn receive(stream: &Stream, expected: &[u8]) -> Result<(), Error> {
    let mut offset = 0;
    while offset < expected.len() {
        let chunk = stream
            .receive(expected.len() - offset)?
            .expect("receive failed");
        let length = usize::try_from(chunk.len()?)?;
        assert!(length > 0);
        let mut bytes = alloc::vec![0; length];
        chunk.read_into(0, &mut bytes)?;
        assert_eq!(bytes, expected[offset..offset + length]);
        offset += length;
    }
    Ok(())
}

#[fixwasm::apply]
fn apply(_: &Handle<Object<Tree>>) -> Result<impl Value, Error> {
    for _ in 0..3 {
        let listener = TcpListener::bind([127, 0, 0, 1], 0)?.expect("bind failed");
        let (ip, port) = listener.local_addr()?.expect("local address failed");
        assert_eq!(ip, [127, 0, 0, 1]);
        assert_ne!(port, 0);
        assert!(TcpListener::bind(ip, port)?.is_none());
        let client = Stream::connect("127.0.0.1", port)?.expect("connect failed");
        let server = listener.accept()?.expect("accept failed");
        listener.close()?;
        let bytes = [0x5a; 1024];
        send(&client, &bytes)?;
        receive(&server, &bytes)?;
        send(&server, b"reply")?;
        receive(&client, b"reply")?;
        client.close()?;
        assert_eq!(server.receive(1)?.expect("EOF failed").len()?, 0);
        server.close()?;
    }
    Ok(Blob::create(b"roundtrip"))
}
