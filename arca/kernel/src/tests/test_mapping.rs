use crate::{cpu::ExitReason, prelude::*};

fn write_mapping(entry: Entry) -> (ExitReason, Arca) {
    let mut code = Page::new(4096);
    code.write(0, &[0x8a, 0x07, 0xc6, 0x07, 0x5a, 0x0f, 0x0b]);
    let mut process = Arca::new();
    assert!(
        process
            .mappings_mut()
            .map(4096, Entry::ROPage(code))
            .is_ok()
    );
    assert!(process.mappings_mut().map(8192, entry).is_ok());
    process.registers_mut()[Register::RIP] = 4096;
    process.registers_mut()[Register::RDI] = 8192;
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
    assert_ne!(
        copy.with_ref(|page| page.as_ptr()),
        source.with_ref(|page| page.as_ptr())
    );
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
    assert_eq!(
        replaced.with_ref(|page| page.as_ptr()),
        page.with_ref(|page| page.as_ptr())
    );
}
