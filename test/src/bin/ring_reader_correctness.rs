/// Reader side of the ring-buffer correctness test.
///
/// Polls for a POSIX shared memory region created by ring_writer_correctness,
/// maps it, and reads `transfer_bytes` from the B-side of the BidirectionalPipe.
/// Prints an XOR checksum of all bytes received.
///
/// Usage: ring_reader_correctness <shm_name> <transfer_bytes> [chunk_bytes]
use memmap2::MmapMut;
use nix::fcntl::OFlag;
use nix::sys::mman::{shm_open, shm_unlink};
use nix::sys::stat::Mode;
use arca_pipe::{BidirectionalPipe, SharedMemoryRegion, Side};
use arca_pipe::Read as PipeRead;

const RING_SIZE: u64 = 64 * 1024;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <shm_name> <transfer_bytes> [chunk_bytes]", args[0]);
        std::process::exit(1);
    }

    let shm_name = format!("/{}", args[1].trim_start_matches('/'));
    let transfer_size: u64 = args[2].parse().expect("transfer_bytes must be a number");
    let chunk_size: usize = args
        .get(3)
        .map(|s| s.parse().expect("chunk_bytes must be a number"))
        .unwrap_or(4096);

    let required = BidirectionalPipe::required_size(RING_SIZE) as usize;

    // Poll until the writer has created the shared memory object.
    println!("Reader: waiting for writer to create shared memory '{}'...", shm_name);
    let fd = loop {
        match shm_open(shm_name.as_str(), OFlag::O_RDWR, Mode::empty()) {
            Ok(fd) => break fd,
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    };
    println!("Reader: shared memory found, mapping {} bytes", required);

    // Safety: fd is valid and points to `required` bytes written by the writer.
    let mut mmap = unsafe { MmapMut::map_mut(&fd).expect("mmap failed") };

    let region = unsafe { SharedMemoryRegion::from_raw(mmap.as_mut_ptr(), required as u64) };
    let mut pipe = BidirectionalPipe::new(&region, RING_SIZE, Side::B);

    let mut buf = vec![0u8; chunk_size];
    let mut pos: u64 = 0;
    let mut xor: u8 = 0;

    while pos < transfer_size {
        let remaining = (transfer_size - pos) as usize;
        let to_read = remaining.min(chunk_size);

        match pipe.read(&mut buf[..to_read]) {
            Ok(n) => {
                for i in 0..n { xor ^= buf[i]; }
                pos += n as u64;
            }
            Err(_) => std::hint::spin_loop(),
        }
    }

    println!("Reader: received {} bytes  XOR=0x{:04X}", pos, xor);

    drop(mmap);
    shm_unlink(shm_name.as_str()).expect("shm_unlink failed");
}
