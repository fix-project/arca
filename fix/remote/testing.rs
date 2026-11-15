use ssh_key::{PrivateKey, Signature, SigningKey, private::Ed25519Keypair};
use std::{
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

pub struct Agent {
    pub socket: PathBuf,
    pub public_key: String,
    pub public_keys: Vec<String>,
    stopping: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Agent {
    pub fn new(socket: &Path, seed: u8) -> Self {
        Self::with_keys(socket, &[seed])
    }

    pub fn with_keys(socket: &Path, seeds: &[u8]) -> Self {
        let keys = seeds
            .iter()
            .map(|seed| PrivateKey::from(Ed25519Keypair::from_seed(&[*seed; 32])))
            .collect::<Vec<_>>();
        let public_keys = keys
            .iter()
            .map(|key| key.public_key().to_openssh().unwrap())
            .collect::<Vec<_>>();
        let public_key = public_keys[0].clone();
        let listener = UnixListener::bind(socket).unwrap();
        let stopping = Arc::new(AtomicBool::new(false));
        let stopped = stopping.clone();
        let thread = std::thread::spawn(move || {
            while let Ok((mut stream, _)) = listener.accept() {
                if stopped.load(Ordering::Acquire) {
                    break;
                }
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                    .unwrap();
                let mut length = [0; 4];
                stream.read_exact(&mut length).unwrap();
                let length = u32::from_be_bytes(length) as usize;
                assert!(length < 4096);
                let mut request = vec![0; length];
                stream.read_exact(&mut request).unwrap();
                assert_eq!(request[0], 13);
                let mut request = &request[1..];
                let identity = string(&mut request);
                let data = string(&mut request);
                assert_eq!(request, &0u32.to_be_bytes());
                let response = if let Some(key) = keys
                    .iter()
                    .find(|key| identity == key.public_key().to_bytes().unwrap())
                {
                    let signature = Vec::<u8>::try_from(sign(key, data)).unwrap();
                    let mut response = vec![14];
                    response.extend_from_slice(&(signature.len() as u32).to_be_bytes());
                    response.extend_from_slice(&signature);
                    response
                } else {
                    vec![5]
                };
                stream
                    .write_all(&(response.len() as u32).to_be_bytes())
                    .unwrap();
                stream.write_all(&response).unwrap();
            }
        });
        Self {
            socket: socket.into(),
            public_key,
            public_keys,
            stopping,
            thread: Some(thread),
        }
    }
}

fn sign(key: &impl SigningKey, message: &[u8]) -> Signature {
    key.try_sign(message).unwrap()
}

fn string<'a>(bytes: &mut &'a [u8]) -> &'a [u8] {
    let length = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
    let value = &bytes[4..4 + length];
    *bytes = &bytes[4 + length..];
    value
}

impl Drop for Agent {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        let _ = UnixStream::connect(&self.socket);
        self.thread.take().unwrap().join().unwrap();
        std::fs::remove_file(&self.socket).unwrap();
    }
}
