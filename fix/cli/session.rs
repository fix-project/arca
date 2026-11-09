use std::{
    io::{Read, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::Arc,
    thread::JoinHandle,
};

pub struct Session {
    stream: UnixStream,
    worker: Option<JoinHandle<u8>>,
}

impl Session {
    pub fn new(
        jobs: usize,
        memory: usize,
        kernel: Arc<[u8]>,
        directory: PathBuf,
    ) -> std::io::Result<Self> {
        let (stream, guest) = UnixStream::pair()?;
        let worker = std::thread::spawn(move || {
            vmm::runtime::Runtime::new(jobs, memory, kernel).run_stream(
                vec!["fix".into(), "--session".into()],
                &directory,
                guest,
            )
        });
        Ok(Self {
            stream,
            worker: Some(worker),
        })
    }

    pub fn execute(
        &mut self,
        arguments: &[String],
        reuse_storage: bool,
    ) -> std::io::Result<(u8, Vec<u8>)> {
        let mut request = vec![u8::from(reuse_storage)];
        for argument in arguments {
            request.extend_from_slice(
                &u32::try_from(argument.len())
                    .map_err(std::io::Error::other)?
                    .to_le_bytes(),
            );
            request.extend_from_slice(argument.as_bytes());
        }
        let length = u32::try_from(request.len()).map_err(std::io::Error::other)?;
        self.stream.write_all(&length.to_le_bytes())?;
        self.stream.write_all(&request)?;
        let mut header = [0; 5];
        self.stream.read_exact(&mut header)?;
        let length = u32::from_le_bytes(header[1..].try_into().unwrap()) as usize;
        let mut output = vec![0; length];
        self.stream.read_exact(&mut output)?;
        Ok((header[0], output))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
