use crate::{cpu::ExitReason, prelude::*};

fn write_program(address: usize, entry: Entry) -> Arca {
    let mut code = Page::new(4096);
    code.write(0, &[0x8a, 0x07, 0xc6, 0x07, 0x5a, 0x0f, 0x0b]);
    let mut process = Arca::new();
    assert!(
        process
            .mappings_mut()
            .map(4096, Entry::ROPage(code))
            .is_ok()
    );
    assert!(process.mappings_mut().map(address, entry).is_ok());
    process.registers_mut()[Register::RIP] = 4096;
    process.registers_mut()[Register::RDI] = address as u64;
    process
}

fn run(process: Arca) -> (ExitReason, Arca) {
    let mut cpu = CPU.borrow_mut();
    let mut process = process.load(&mut cpu);
    let result = loop {
        match process.run() {
            ExitReason::Interrupted(0x20) => continue,
            result => break result,
        }
    };
    (result, process.unload())
}

fn write_mapping(entry: Entry) -> (ExitReason, Arca) {
    run(write_program(8192, entry))
}

#[test]
fn snapshots_isolate_writes() {
    for size in [4096, 1 << 21] {
        let address = if size == 4096 { 8192 } else { size };
        let mut page = Page::new(size);
        page.write(0, &[0x29]);
        let original = write_program(address, Entry::RWPage(page));
        let snapshot = original.clone();
        let (result, snapshot) = run(snapshot);
        assert_eq!(result, ExitReason::InvalidInstruction);
        assert_eq!(snapshot.registers()[Register::RAX] & 0xff, 0x29);
        let mut byte = [0];
        assert_eq!(original.mappings().inner().read(address, &mut byte), 1);
        assert_eq!(byte, [0x29]);
        let (result, original) = run(original);
        assert_eq!(result, ExitReason::InvalidInstruction);
        assert_eq!(original.registers()[Register::RAX] & 0xff, 0x29);
        assert_eq!(original.mappings().inner().read(address, &mut byte), 1);
        assert_eq!(byte, [0x5a]);
    }
}

#[test]
fn zeroed_tables_isolate_writes_across_shared_levels() {
    let address = 1 << 36;
    for size in [1 << 21, 1 << 30] {
        let entry = <Runtime as arca::Runtime>::create_zeroed_entry(size);
        let original = write_program(address, entry);
        for offset in [0, 4096, size / 512, size / 2, size - 4096] {
            let mut snapshot = original.clone();
            snapshot.registers_mut()[Register::RDI] = (address + offset) as u64;
            let (result, snapshot) = run(snapshot);
            assert_eq!(result, ExitReason::InvalidInstruction);
            assert_eq!(snapshot.registers()[Register::RAX] & 0xff, 0);
            let mut bytes = [0; 2];
            assert_eq!(
                snapshot
                    .mappings()
                    .inner()
                    .read(address + offset, &mut bytes),
                2
            );
            assert_eq!(bytes, [0x5a, 0]);
            assert_eq!(
                original
                    .mappings()
                    .inner()
                    .read(address + offset, &mut bytes),
                2
            );
            assert_eq!(bytes, [0; 2]);
        }
        let fresh = write_program(
            address,
            <Runtime as arca::Runtime>::create_zeroed_entry(size),
        );
        let mut byte = [1];
        assert_eq!(
            fresh.mappings().inner().read(address + size - 1, &mut byte),
            1
        );
        assert_eq!(byte, [0]);
    }
}

#[test]
fn writable_shared_tables_allow_first_write() {
    let address = 1 << 30;
    for size in [1 << 21, 1 << 30] {
        let mut table = Table::new(size);
        let mut page = Page::new(4096);
        page.write(0, &[0x29]);
        assert!(table.map(0, Entry::RWPage(page)).is_ok());
        let table = Table::from_inner(table.into_inner().shared());
        let (result, process) = run(write_program(address, Entry::RWTable(table)));
        assert_eq!(result, ExitReason::InvalidInstruction);
        assert_eq!(process.registers()[Register::RAX] & 0xff, 0x29);
        let mut byte = [0];
        assert_eq!(process.mappings().inner().read(address, &mut byte), 1);
        assert_eq!(byte, [0x5a]);
    }
}

#[test]
fn cow_tables_preserve_read_only_leaves() {
    let Entry::RWTable(mut table) = <Runtime as arca::Runtime>::create_zeroed_entry(1 << 21) else {
        panic!()
    };
    let mut page = Page::new(4096);
    page.write(0, &[0x29]);
    table.set(0, Entry::ROPage(page)).unwrap();
    let table = Table::from_inner(table.into_inner().shared());
    let (result, process) = run(write_program(1 << 21, Entry::RWTable(table)));
    assert_eq!(
        result,
        ExitReason::PageFault {
            addr: 1 << 21,
            error: 7
        }
    );
    assert_eq!(process.registers()[Register::RAX] & 0xff, 0x29);
}

#[test]
fn writable_mapping_reinsertion_preserves_snapshot_isolation() {
    let mut page = Page::new(4096);
    page.write(0, &[0x29]);
    let original = write_program(8192, Entry::RWPage(page));
    let mut snapshot = original.clone();
    let page = snapshot.mappings_mut().unmap(8192).unwrap();
    assert!(matches!(page, Entry::RWPage(_)));
    assert!(snapshot.mappings_mut().map(8192, page).is_ok());
    let (result, snapshot) = run(snapshot);
    assert_eq!(result, ExitReason::InvalidInstruction);
    let mut byte = [0];
    assert_eq!(snapshot.mappings().inner().read(8192, &mut byte), 1);
    assert_eq!(byte, [0x5a]);
    assert_eq!(original.mappings().inner().read(8192, &mut byte), 1);
    assert_eq!(byte, [0x29]);
}

#[test]
fn concurrent_snapshot_clones_isolate_writes() {
    use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    let mut page = Page::new(4096);
    page.write(0, &[0x29]);
    let original = Arc::new(write_program(8192, Entry::RWPage(page)));
    let ready = Arc::new(AtomicUsize::new(0));
    let start = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicUsize::new(0));
    for _ in 0..4 {
        let original = original.clone();
        let ready = ready.clone();
        let start = start.clone();
        let finished = finished.clone();
        crate::kthread::spawn(move || {
            ready.fetch_add(1, Ordering::Release);
            while !start.load(Ordering::Acquire) {
                crate::kthread::yield_now();
            }
            for _ in 0..32 {
                let (result, snapshot) = run((*original).clone());
                assert_eq!(result, ExitReason::InvalidInstruction);
                assert_eq!(snapshot.registers()[Register::RAX] & 0xff, 0x29);
                let mut byte = [0];
                assert_eq!(snapshot.mappings().inner().read(8192, &mut byte), 1);
                assert_eq!(byte, [0x5a]);
            }
            finished.fetch_add(1, Ordering::Release);
        });
    }
    while ready.load(Ordering::Acquire) != 4 {
        crate::kthread::yield_now();
    }
    start.store(true, Ordering::Release);
    while finished.load(Ordering::Acquire) != 4 {
        crate::kthread::yield_now();
    }
    let mut byte = [0];
    assert_eq!(original.mappings().inner().read(8192, &mut byte), 1);
    assert_eq!(byte, [0x29]);
}

#[test]
fn read_only_ancestors_reject_writes_to_cow_leaves() {
    let mut page = Page::new(4096);
    page.write(0, &[0x29]);
    let mut table = Table::new(1 << 21);
    assert!(table.map(0, Entry::RWPage(page)).is_ok());
    drop(table.clone());
    let process = write_program(1 << 21, Entry::ROTable(table));
    let (result, process) = run(process);
    assert_eq!(
        result,
        ExitReason::PageFault {
            addr: 1 << 21,
            error: 7
        }
    );
    assert_eq!(process.registers()[Register::RAX] & 0xff, 0x29);
}

#[test]
fn kernel_copyout_isolates_cow_pages_across_a_boundary() {
    for shared_table in [false, true] {
        let destination = if shared_table {
            (1 << 30) + (1 << 21) - 4
        } else {
            0x2ffcu32
        };
        let mut bytes = [
            0xb8,
            arcane::__NR_get_argument as u8,
            0,
            0,
            0,
            0x0f,
            0x05,
            0x48,
            0x89,
            0xc7,
            0x31,
            0xf6,
            0xba,
            0xfc,
            0x2f,
            0,
            0,
            0x41,
            0xba,
            8,
            0,
            0,
            0,
            0xb8,
            arcane::__NR_read as u8,
            0,
            0,
            0,
            0x0f,
            0x05,
            0x48,
            0x89,
            0xc3,
            0xb8,
            arcane::__NR_get_argument as u8,
            0,
            0,
            0,
            0x0f,
            0x05,
        ];
        bytes[13..17].copy_from_slice(&destination.to_le_bytes());
        let mut code = Page::new(4096);
        code.write(0, &bytes);
        let mut original = Arca::new();
        assert!(
            original
                .mappings_mut()
                .map(4096, Entry::ROPage(code))
                .is_ok()
        );
        if shared_table {
            assert!(
                original
                    .mappings_mut()
                    .map(
                        1 << 30,
                        <Runtime as arca::Runtime>::create_zeroed_entry(1 << 30)
                    )
                    .is_ok()
            );
        } else {
            for address in [8192, 12288] {
                assert!(
                    original
                        .mappings_mut()
                        .map(address, Entry::RWPage(Page::new(4096)))
                        .is_ok()
                );
            }
        }
        original.registers_mut()[Register::RIP] = 4096;
        for value in [7, 19] {
            let snapshot = original.clone();
            let function: Function = snapshot.into();
            let function: Function = function
                .apply(Blob::new([value; 8]))
                .force()
                .try_into()
                .unwrap();
            let mut function = function.into_inner();
            let process = function.arca_mut().unwrap();
            assert_eq!(process.registers()[Register::RBX], 8);
            let mut bytes = [0; 8];
            assert_eq!(
                process
                    .mappings()
                    .inner()
                    .read(destination as usize, &mut bytes),
                8
            );
            assert_eq!(bytes, [value; 8]);
            assert_eq!(
                original
                    .mappings()
                    .inner()
                    .read(destination as usize, &mut bytes),
                8
            );
            assert_eq!(bytes, [0; 8]);
        }
    }
}

#[test]
fn immutable_mapping_and_writable_copy() {
    let mut source = Page::new(4096);
    assert_eq!(source.write(0, &[0x29]), 1);
    let source = Page::from_inner(source.into_inner().shared());
    let (result, process) = write_mapping(Entry::ROPage(source.clone()));
    assert!(matches!(
        result,
        ExitReason::PageFault {
            addr: 8192,
            error: 7
        }
    ));
    assert_eq!(process.registers()[Register::RIP], 4098);
    assert_eq!(process.registers()[Register::RAX] & 0xff, 0x29);

    let (result, process) = write_mapping(Entry::RWPage(source.clone()));
    assert_eq!(result, ExitReason::InvalidInstruction);
    let Entry::RWPage(copy) = process.mappings().get(2).unwrap() else {
        panic!()
    };
    assert_eq!(copy.with_ref(|page| page[0]), 0x5a);
    assert_eq!(source.with_ref(|page| page[0]), 0x29);
}

#[test]
fn nested_mapping_returns_replaced_entry() {
    let mut page = Page::new(4096);
    page.write(0, &[0x29]);
    let page = Page::from_inner(page.into_inner().shared());
    let mut table = Table::new(1 << 30);
    assert!(matches!(
        table.map(1 << 21, Entry::ROPage(page.clone())),
        Ok(Entry::Null(4096))
    ));
    let Ok(Entry::ROPage(replaced)) = table.map(1 << 21, Entry::Null(4096)) else {
        panic!("expected the replaced Page");
    };
    assert_eq!(replaced.with_ref(|page| page[0]), 0x29);
}

#[test]
fn interleaved_continuations_preserve_different_address_space_sizes() {
    let addresses = [8192, 1 << 21, 1 << 30];
    let mut functions: Vec<_> = addresses
        .iter()
        .enumerate()
        .map(|(index, &address)| {
            let mut code = Page::new(4096);
            code.write(
                0,
                &[
                    0x8a,
                    0x1f,
                    0xc6,
                    0x07,
                    0x5a,
                    0xb8,
                    arcane::__NR_get_argument as u8,
                    0,
                    0,
                    0,
                    0x0f,
                    0x05,
                    0xeb,
                    0xf2,
                ],
            );
            let mut data = Page::new(4096);
            data.write(0, &[index as u8]);
            let mut process = Arca::new();
            assert!(
                process
                    .mappings_mut()
                    .map(4096, Entry::ROPage(code))
                    .is_ok()
            );
            assert!(
                process
                    .mappings_mut()
                    .map(address, Entry::RWPage(data))
                    .is_ok()
            );
            process.registers_mut()[Register::RIP] = 4096;
            process.registers_mut()[Register::RDI] = address as u64;
            Some(Function::from_inner(
                crate::types::internal::Function::arcane_with_args(process, Default::default()),
            ))
        })
        .collect();
    for round in 0..3 {
        for (index, &address) in addresses.iter().enumerate() {
            let result: Function = functions[index].take().unwrap().force().try_into().unwrap();
            let mut result = result.into_inner();
            let process = result.arca_mut().unwrap();
            let expected = if round == 0 { index as u64 } else { 0x5a };
            assert_eq!(process.registers()[Register::RBX] & 0xff, expected);
            let Some(Entry::RWPage(data)) = process.mappings_mut().unmap(address) else {
                panic!()
            };
            assert_eq!(data.with_ref(|bytes| bytes[0]), 0x5a);
            assert!(
                process
                    .mappings_mut()
                    .map(address, Entry::RWPage(data))
                    .is_ok()
            );
            functions[index] = Some(Function::from_inner(result));
        }
    }
}
