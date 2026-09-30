use crate::{
    host::fs::{File, Whence},
    interrupts::IO_GENERATION,
    kthread,
};
use core::sync::atomic::Ordering;

#[test]
fn interrupt_between_readiness_check_and_park() {
    for _ in 0..100 {
        let mut checked = false;
        kthread::wait_until(|| {
            if checked {
                true
            } else {
                checked = true;
                // Model an IRQ arriving after observing an empty pipe, before
                // the scheduler can park this thread. No later IRQ is needed.
                IO_GENERATION.fetch_add(1, Ordering::SeqCst);
                false
            }
        });
        // A resumed I/O waiter must be able to yield normally afterwards.
        kthread::yield_now();
    }
}

#[test]
fn file_io_with_backpressure() {
    let bytes = [0xa5; 8192]; // much larger than the file pipe's 1024-byte ring
    for _ in 0..20 {
        let mut file = File::open("interrupt-io-test", true, true, true, false, true).unwrap();
        assert_eq!(file.write_exact(&bytes), bytes.len());
        assert_eq!(file.seek(Whence::Start(0)), 0);
        let mut response = [0; 8192];
        assert_eq!(file.read_exact(&mut response), response.len());
        assert_eq!(response, bytes);
        file.close();
    }
}
