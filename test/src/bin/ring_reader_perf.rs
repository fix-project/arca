/// Reader side of the ring-buffer throughput test.
///
/// Polls for the POSIX shared memory region created by ring_writer_perf, maps
/// it, signals the writer to start, then reads `transfer_bytes` and prints
/// GB/s throughput.
///
/// Usage: ring_reader_perf <shm_name> <transfer_bytes> [chunk_bytes]
use std::hint::spin_loop;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use memmap2::MmapMut;
use nix::fcntl::OFlag;
use nix::sys::mman::{shm_open, shm_unlink};
use nix::sys::stat::Mode;
use nix::unistd::{sysconf, SysconfVar};
use arca_pipe::{BidirectionalPipe, SharedMemoryRegion, Side};
use arca_pipe::Read as PipeRead;

const RING_SIZE: u64 = 1024 * 1024;
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

    println!("Reader: waiting for writer to create shared memory '{}'...", shm_name);
    let fd = loop {
        match shm_open(shm_name.as_str(), OFlag::O_RDWR, Mode::empty()) {
            Ok(fd) => break fd,
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    };
    println!("Reader: shared memory found, mapping {} bytes", total_size);

    let mut mmap = unsafe { MmapMut::map_mut(&fd).expect("mmap failed") };

    // store CTRL_BYTES on shared mem
    let ready: &AtomicU64 = unsafe { &*(mmap.as_ptr() as *const AtomicU64) };
    let region = unsafe {
        SharedMemoryRegion::from_raw(mmap.as_mut_ptr().add(CTRL_BYTES), pipe_size as u64)
    };
    let mut pipe = BidirectionalPipe::new(&region, RING_SIZE, Side::B);

    let mut dst = vec![0u8; transfer_size as usize];

    // Pre-fault all pages so allocation cost doesn't hit the timed path.
    let page_size = sysconf(SysconfVar::PAGE_SIZE).unwrap().unwrap() as usize;
    for i in (0..dst.len()).step_by(page_size) {
        dst[i] = 1;
    }

    let ckpt_total: u64 = 10;
    let ckpt_sz = (transfer_size + ckpt_total - 1) / ckpt_total;
    let mut ckpt_next = ckpt_sz;

    // Signal writer to start.
    ready.store(1, Ordering::Release);
    println!("Reader: signaled writer, waiting for data...");

    let start = Instant::now();
    eprintln!("--- Reader checkpoint 0/{}", ckpt_total);

    let mut read: u64 = 0;
    while read < transfer_size {
        let remaining = (transfer_size - read) as usize;
        let to_read = remaining.min(chunk_size);
        match pipe.read(&mut dst[read as usize..read as usize + to_read]) {
            Ok(n) => {
                read += n as u64;
                if read >= ckpt_next {
                    eprintln!(
                        "--- Reader checkpoint {}/{} elapsed: {:.3}s",
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

    println!("========================================");
    println!("READER STATS");
    println!("========================================");
    println!("Total time: {} µs  {:.6} s", elapsed.as_micros(), elapsed.as_secs_f64());
    println!(
        "Throughput: {:.4} GB/s",
        read as f64 / (1024.0 * 1024.0 * 1024.0 * elapsed.as_secs_f64())
    );
    println!("========================================");

    // Signal writer we are done so it can unmap cleanly.
    ready.store(0, Ordering::Relaxed);

    drop(mmap);
    shm_unlink(shm_name.as_str()).expect("shm_unlink failed");
}
