use crate::{
    host::fs::{File, Whence},
    kthread,
};

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
    for worker in 0..4 {
        let completed = completed.clone();
        kthread::spawn(move || {
            let path = alloc::format!("concurrent-io-{worker}");
            let mut file = File::open(&path, true, true, true, false, true).unwrap();
            let bytes = Buffer::from(Box::new_in([worker as u8; 1024], BuddyAllocator));
            let mut response = Buffer::from(Box::new_in([0u8; 1024], BuddyAllocator));
            for _ in 0..8 {
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
    while *count != 4 {
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

#[test]
fn owned_file_buffers_preserve_contents_across_heap_growth() {
    use crate::{host::fs, prelude::*};

    for length in [0, 37, 4097] {
        let expected: Vec<u8> = (0..length).map(|index| index as u8).collect();
        fs::write("owned-buffer-test", &expected).unwrap();
        let mut actual = fs::read("owned-buffer-test").unwrap();
        assert_eq!(actual, expected);
        actual.reserve(length + 1);
        actual.extend_from_slice(&expected);
        assert_eq!(&actual[..length], &expected);
        assert_eq!(&actual[length..], &expected);
    }
}
