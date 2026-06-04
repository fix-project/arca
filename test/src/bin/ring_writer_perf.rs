/// Writer side of the ring-buffer throughput test.
///
/// Creates a POSIX shared memory region with a ready flag followed by
/// a BidirectionalPipe (Side::A). Waits for the reader to signal ready, then
/// streams `transfer_bytes` and prints GB/s throughput.
///
/// Usage: ring_writer_perf <shm_name> <transfer_bytes> [chunk_bytes]
use std::hint::spin_loop;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use memmap2::MmapMut;
use nix::fcntl::OFlag;
use nix::sys::mman::shm_open;
use nix::sys::stat::Mode;
use nix::unistd::ftruncate;
use arca_pipe::{BidirectionalPipe, SharedMemoryRegion, Side};
use arca_pipe::Write as PipeWrite;

const RING_SIZE: u64 = 1024 * 1024;
// One cache line of control space before the ring region to avoid false sharing.
const CTRL_BYTES: usize = 64;

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

    let pipe_size = BidirectionalPipe::required_size(RING_SIZE) as usize;
    let total_size = CTRL_BYTES + pipe_size;

    let fd = shm_open(
        shm_name.as_str(),
        OFlag::O_CREAT | OFlag::O_RDWR,
        Mode::from_bits_truncate(0o666),
    )
    .expect("shm_open failed");

    ftruncate(&fd, total_size as i64).expect("ftruncate failed");

    let mut mmap = unsafe { MmapMut::map_mut(&fd).expect("mmap failed") };

    // Zero out the entire region before use
    mmap.fill(0);

    let ready: &AtomicU64 = unsafe { &*(mmap.as_ptr() as *const AtomicU64) };
    let region = unsafe {
        SharedMemoryRegion::from_raw(mmap.as_mut_ptr().add(CTRL_BYTES), pipe_size as u64)
    };
    let mut pipe = BidirectionalPipe::new(&region, RING_SIZE, Side::A);

    // hard-coded buffer to transfer
    let buf: Vec<u8> = (0..chunk_size).map(|i| ((i % 255) + 1) as u8).collect();

    let ckpt_total: u64 = 10;
    let ckpt_sz = (transfer_size + ckpt_total - 1) / ckpt_total;
    let mut ckpt_next = ckpt_sz;

    println!("Writer: waiting for reader to signal ready...");
    while ready.load(Ordering::Acquire) == 0 {
        spin_loop();
    }

    println!("Writer: reader ready, starting write of {} bytes", transfer_size);
    let start = Instant::now();
    eprintln!("--- Writer checkpoint 0/{}", ckpt_total);

    let mut written: u64 = 0;
    while written < transfer_size {
        let remaining = (transfer_size - written) as usize;
        let to_write = remaining.min(chunk_size);
        match pipe.write(&buf[..to_write]) {
            Ok(n) => {
                written += n as u64;
                if written >= ckpt_next {
                    eprintln!(
                        "--- Writer checkpoint {}/{} elapsed: {:.3}s",
                        ckpt_next / ckpt_sz,
                        ckpt_total,
                        start.elapsed().as_secs_f64()
                    );
                    ckpt_next += ckpt_sz;
                }
            }
            Err(_) => spin_loop(),
        }
    }

    let elapsed = start.elapsed();

    // Wait for reader to finish draining before unmapping.
    while ready.load(Ordering::Relaxed) != 0 {
        spin_loop();
    }

    println!("========================================");
    println!("WRITER STATS");
    println!("========================================");
    println!("Total time: {} µs  {:.6} s", elapsed.as_micros(), elapsed.as_secs_f64());
    println!("Data written: {} bytes", written);
    println!(
        "Throughput: {:.4} GB/s",
        written as f64 / (1024.0 * 1024.0 * 1024.0 * elapsed.as_secs_f64())
    );
    println!("========================================");

    // Reader is responsible for shm_unlink.
    drop(mmap);
}
