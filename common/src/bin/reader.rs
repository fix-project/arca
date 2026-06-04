// reader.rs
use std::env;
use std::ffi::CString;
use std::sync::atomic::{fence, Ordering};
use std::ptr;
use std::mem::size_of;
use throughput::{read_tsc, ShmHeader};

const MB: u64 = 1024 * 1024;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 5 {
        eprintln!(
            "Usage: {} <shared_mem_name> <share_mem_size_bytes> <transfer_size_mb> <read_chunk_size_bytes>",
            args[0]
        );
        std::process::exit(1);
    }

    let shm_name = &args[1];
    let shm_size: u64 = args[2].parse()
        .expect("share_mem_size must be a valid number (bytes)");
    let transfer_size_mb: u64 = args[3].parse()
        .expect("transfer_size_mb must be a valid number (MB)");
    let transfer_size: u64 = transfer_size_mb.saturating_mul(MB);
    let chunk_size: u32 = args[4].parse()
        .expect("chunk_size must be a valid number (bytes)");

    // Add '/' prefix if needed
    let shm_name = if shm_name.starts_with('/') {
        shm_name.to_string()
    } else {
        format!("/{}", shm_name)
    };

    // Convert to C string
    let c_name = CString::new(shm_name.as_bytes()).unwrap();

    println!("Reader: Waiting for writer to create shared memory...");

    // Open existing shared memory (no O_CREAT flag)
    let fd = loop {
        let fd = unsafe { libc::shm_open(c_name.as_ptr(), libc::O_RDWR, 0o666) };
        if fd >= 0 {
            break fd;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    };

    println!("Reader: Shared memory found!");

    let total_size = size_of::<ShmHeader>() as u64 + shm_size;
    println!("Writer: ShmHeader size: {}", size_of::<ShmHeader>());

    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            total_size as usize,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd,
            0,
        )
    };

    if ptr == libc::MAP_FAILED {
        panic!("Failed to map shared memory");
    }

    // Get pointers to shared variables
    let header = unsafe { &*(ptr as *mut ShmHeader) };
    let data_start = unsafe { (ptr as *mut u8).add(size_of::<ShmHeader>()) };

    // Prepare buffer for reading
    let mut dst = vec![0u8; transfer_size as usize];

    // Touch pages so allocation/fault cost doesn't hit the timed path (reader side)
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize };
    for i in (0..dst.len()).step_by(page_size) {
        dst[i] = 1;
    }

    let mut total_read = 0u64;

    #[cfg(debug_assertions)]
    let mut xor_checksum: u8 = 0;

    // tsc
    let ckpt_total_interval = 10;
    let ckpt_interval_sz = (transfer_size + ckpt_total_interval - 1) / ckpt_total_interval;
    let mut ckpt_next = ckpt_interval_sz;

    // Change transfer_started to 1 (signal writer to start)
    header.transfer_started.store(1, Ordering::Release);
    println!("Reader: Signaled writer to start, waiting for data...");

    eprintln!("--- Reader checkpoint 0/{} tsc: {}", ckpt_total_interval, read_tsc());

    while total_read < transfer_size {
        let end_idx = header.end_index.load(Ordering::Acquire);
        let start_idx = header.start_index.load(Ordering::Acquire);

        let avail_len = end_idx - start_idx;

        if avail_len > 0 {
            let len = (chunk_size as u64)
                .min(transfer_size - total_read)
                .min(avail_len);

            // Calculate read position with wrap-around
            let read_start = (start_idx % shm_size) as usize;
            let l = std::cmp::min(len, shm_size - read_start as u64) as usize;

            unsafe {
                // First part (until wrap or end of chunk)
                ptr::copy_nonoverlapping(
                    data_start.add(read_start),
                    dst.as_mut_ptr().add(total_read as usize),
                    l,
                );

                // Second part (wrapped around to beginning)
                if l < len as usize {
                    ptr::copy_nonoverlapping(
                        data_start,
                        dst.as_mut_ptr().add(total_read as usize + l),
                        len as usize - l,
                    );
                }
            }

            // Barrier: smp_wmb()
            fence(Ordering::Release);

            header.start_index.store(start_idx + len, Ordering::Relaxed);
            total_read += len;

            if total_read > ckpt_next {
                eprintln!(
                    "--- Reader checkpoint {}/{} tsc: {}",
                    ckpt_next / ckpt_interval_sz,
                    ckpt_total_interval,
                    read_tsc()
                );
                ckpt_next += ckpt_interval_sz;
            }
        } else {
            std::hint::spin_loop();
        }
    }

    eprintln!(
        "--- Reader checkpoint {}/{} tsc: {}",
        ckpt_next / ckpt_interval_sz,
        ckpt_total_interval,
        read_tsc()
    );
    println!("Reader: Finished reading {} bytes", total_read);

    header.transfer_started.store(0, Ordering::Relaxed);

    #[cfg(debug_assertions)]
    {
        for i in 0..total_read as usize {
            xor_checksum ^= dst[i];
        }
        println!("Reader XOR checksum: 0x{:02X}", xor_checksum);
    }

    // Cleanup
    unsafe {
        libc::munmap(ptr, total_size as usize);
        libc::close(fd);
        libc::shm_unlink(c_name.as_ptr());
    }
}