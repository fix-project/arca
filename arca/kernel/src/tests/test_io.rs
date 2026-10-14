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

#[test]
fn concurrent_file_io() {
    use crate::{host::fs, prelude::*};
    use common::buffer::Buffer;

    let completed = Arc::new((kthread::KMutex::new(0), kthread::KCondvar::new()));
    for worker in 0..8 {
        let completed = completed.clone();
        kthread::spawn(move || {
            let path = alloc::format!("concurrent-io-{worker}");
            let mut file = File::open(&path, true, true, true, false, true).unwrap();
            let bytes = Buffer::from(Box::new_in([worker as u8; 1024], BuddyAllocator));
            let mut response = Buffer::from(Box::new_in([0u8; 1024], BuddyAllocator));
            for _ in 0..128 {
                assert!(fs::exists(&path).unwrap());
                file.seek(Whence::Start(0));
                assert_eq!(file.write_exact(&bytes), bytes.len());
                file.seek(Whence::Start(0));
                assert_eq!(file.read_exact(&mut response), bytes.len());
                assert_eq!(&*response, &*bytes);
            }
            file.close();
            fs::remove_file(&path).unwrap();
            *completed.0.lock() += 1;
            completed.1.notify_one();
        });
    }
    let mut count = completed.0.lock();
    while *count != 8 {
        count = completed.1.wait(count);
    }
}

#[test]
fn whole_file_io() {
    use crate::host::fs;
    use crate::prelude::*;
    use common::protocol::control::{ErrorKind, FileMode};

    let path = "whole-file-io-test";
    for length in [128 * 1024 + 1, 31, 0] {
        let bytes: Vec<_> = (0..length).map(|index| index as u8).collect();
        fs::write(path, &bytes).unwrap();
        assert!(fs::exists(path).unwrap());
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    assert_eq!(
        fs::write_with(
            path,
            b"replacement",
            FileMode {
                write: true,
                create_new: true,
                ..FileMode::default()
            }
        ),
        Err(ErrorKind::AlreadyExists)
    );
    assert!(fs::read(path).unwrap().is_empty());
    fs::remove_file(path).unwrap();
    assert!(!fs::exists(path).unwrap());
    assert_eq!(fs::read(path), Err(ErrorKind::NotFound));
    fs::symlink("missing-whole-file-io-test", path).unwrap();
    assert!(!fs::exists(path).unwrap());
    fs::remove_file(path).unwrap();
}
