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
fn file_io_lends_unique_and_shared_memory() {
    use crate::prelude::*;

    let mut bytes = Box::new_in([0u8; 128 * 1024], BuddyAllocator);
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = i as u8;
    }
    let bytes: RefCnt<_> = bytes.into();
    let shared = common::buffer::Buffer::from(bytes.clone());
    let mut file = File::open("interrupt-io-test", true, true, true, false, true).unwrap();
    assert_eq!(file.write_exact(&shared), shared.len());
    assert_eq!(file.seek(Whence::Start(0)), 0);
    let mut response =
        common::buffer::Buffer::from(Box::new_in([0xa5; 128 * 1024 + 16], BuddyAllocator));
    assert_eq!(file.read_exact(&mut response.slice_mut(8..)), bytes.len());
    assert_eq!(&response[8..8 + bytes.len()], &bytes[..]);
    assert_eq!(&response[..8], &[0xa5; 8]);
    assert_eq!(&response[8 + bytes.len()..], &[0xa5; 8]);
    assert_eq!(file.read(&mut response), 0);
    assert_eq!(file.read(&mut response.slice_mut(..0)), 0);
    assert_eq!(file.write(&shared.slice(..0)), 0);
    file.close();
}
