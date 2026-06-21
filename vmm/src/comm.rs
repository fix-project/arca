use crate::pipe::GuestPipe;
use common::protocol::*;

pub fn communication_thread(argv: Vec<String>, mut pipe: GuestPipe) {
    loop {
        let mut length = [0; 8];
        pipe.read_exact(&mut length).unwrap();
        let length = usize::from_le_bytes(length);
        let mut bytes = vec![0; length];
        pipe.read_exact(&mut bytes).unwrap();
        let ControlRequest::GetArgs = postcard::from_bytes(&bytes).unwrap();
        let bytes = postcard::to_allocvec(&ControlResponse::Args(argv.clone())).unwrap();
        let length = bytes.len().to_le_bytes();
        pipe.write_exact(&length).unwrap();
        pipe.write_exact(&bytes).unwrap();
    }
}
