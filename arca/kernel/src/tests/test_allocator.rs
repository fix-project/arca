use crate::{kthread, prelude::*};

#[repr(align(512))]
struct Aligned([u8; 512]);

#[test]
fn heap_objects_preserve_contents_across_threads_and_growth() {
    let completed = Arc::new((kthread::KMutex::new(0), kthread::KCondvar::new()));
    for worker in 0..4 {
        let blocks: Vec<_> = [1, 37, 4097]
            .into_iter()
            .map(|length| vec![worker as u8; length])
            .collect();
        let aligned = Box::new(Aligned([worker as u8; 512]));
        let completed = completed.clone();
        kthread::spawn(move || {
            assert!((&raw const *aligned as usize).is_multiple_of(512));
            assert!(aligned.0.iter().all(|byte| *byte == worker as u8));
            for mut block in blocks {
                let length = block.len();
                assert!(block.iter().all(|byte| *byte == worker as u8));
                block.resize(length * 2 + 1, 0xa5);
                assert!(block[..length].iter().all(|byte| *byte == worker as u8));
                assert!(block[length..].iter().all(|byte| *byte == 0xa5));
            }
            *completed.0.lock() += 1;
            completed.1.notify_all();
        });
    }
    let mut count = completed.0.lock();
    while *count != 4 {
        count = completed.1.wait(count);
    }
}
