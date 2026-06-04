/// Writer side of the ring-buffer correctness test.
///
/// Creates a POSIX shared memory region, lays out a BidirectionalPipe over it
/// (Side::A), and streams `transfer_bytes` of a fixed pattern into the A→B
/// channel. Prints an XOR checksum of all bytes sent.
///
/// Usage: ring_writer_correctness <shm_name> <transfer_bytes> [chunk_bytes]
use memmap2::MmapMut;
use nix::fcntl::OFlag;
use nix::sys::mman::shm_open;
use nix::sys::stat::Mode;
use nix::unistd::ftruncate;
use arca_pipe::{BidirectionalPipe, SharedMemoryRegion, Side};
use arca_pipe::Write as PipeWrite;

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

    let fd = shm_open(
        shm_name.as_str(),
        OFlag::O_CREAT | OFlag::O_RDWR,
        Mode::from_bits_truncate(0o666),
    )
    .expect("shm_open failed");

    ftruncate(&fd, required as i64).expect("ftruncate failed");

    // Safety: fd is valid and points to `required` zero-initialised bytes.
    let mut mmap = unsafe { MmapMut::map_mut(&fd).expect("mmap failed") };

    // Zero out to clear any stale cursor state if shm name was reused.
    mmap.fill(0);

    let region = unsafe { SharedMemoryRegion::from_raw(mmap.as_mut_ptr(), required as u64) };
    let mut pipe = BidirectionalPipe::new(&region, RING_SIZE, Side::A);

    let buf: Vec<u8> = (0..chunk_size).map(|i| ((i % 255) + 1) as u8).collect();
    let mut pos: u64 = 0;
    let mut xor: u8 = 0;

    println!("Writer: shared memory created ({}), starting write of {} bytes", shm_name, transfer_size);

    while pos < transfer_size {
        let remaining = (transfer_size - pos) as usize;
        let to_write = remaining.min(chunk_size);

        match pipe.write(&buf[..to_write]) {
            Ok(n) => {
                for i in 0..n { xor ^= buf[i]; }
                pos += n as u64;
            }
            Err(_) => std::hint::spin_loop(),
        }
    }

    println!("Writer: sent {} bytes  XOR=0x{:04X}", pos, xor);

    // Reader is responsible for shm_unlink.
    drop(mmap);
}
