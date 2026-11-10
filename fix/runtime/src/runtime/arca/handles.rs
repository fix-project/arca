use super::*;
use crate::storage::ImmutableBytes;
use kernel::prelude::{Entry, Page};

pub(super) const HANDLE_BASE: usize = 0x7e00000000;
const HANDLE_END: usize = HANDLE_BASE + (1 << 32);

pub(super) fn read_backing(backing: &ImmutableBytes, index: usize) -> Handle {
    let offset = index
        .checked_mul(HANDLE_SIZE)
        .expect("handle index overflow");
    assert!(offset < backing.len() && backing.len() - offset >= HANDLE_SIZE);
    let (page, position) = backing.page(offset);
    let mut bytes = [0; HANDLE_SIZE];
    assert_eq!(page.read(position, &mut bytes), HANDLE_SIZE);
    unsafe { Handle::parse(bytes) }.expect("invalid stored handle")
}

pub(super) fn reference(bytes: &[u8; 8]) -> usize {
    let address = u64::from_le_bytes(*bytes) as usize;
    assert!((HANDLE_BASE..HANDLE_END).contains(&address));
    assert!(address.is_multiple_of(HANDLE_SIZE));
    address
}

pub(super) fn read(function: &mut Function, address: usize, bytes: &mut [u8]) -> usize {
    let mappings = function.inner_mut().arca_mut().unwrap().mappings();
    mappings.inner().read(address, bytes)
}

pub(super) fn resolve(function: &mut Function, address: usize) -> Handle {
    assert!((HANDLE_BASE..HANDLE_END).contains(&address));
    assert!(address.is_multiple_of(HANDLE_SIZE));
    let mut bytes = [0; HANDLE_SIZE];
    assert_eq!(read(function, address, &mut bytes), HANDLE_SIZE);
    unsafe { Handle::parse(bytes) }.expect("invalid handle cell")
}

pub(super) fn insert(function: &mut Function, address: usize, handle: Handle) -> usize {
    assert!((HANDLE_BASE..HANDLE_END).contains(&address));
    assert!(address.is_multiple_of(HANDLE_SIZE));
    let mappings = function.inner_mut().arca_mut().unwrap().mappings_mut();
    let result = mappings.update(address & !4095, 4096, |entry| {
        let mut page = match entry {
            Entry::ROPage(page) | Entry::RWPage(page) => page,
            Entry::Null(_) => Page::new(4096),
            _ => panic!("invalid handle mapping"),
        };
        assert_eq!(page.write(address % 4096, handle.as_bytes()), HANDLE_SIZE);
        (Entry::RWPage(page), ())
    });
    assert!(result.is_ok());
    address
}

pub(super) fn map_backing(
    function: &mut Function,
    address: usize,
    backing: &ImmutableBytes,
    writable: bool,
) {
    assert!(address.is_multiple_of(4096));
    for offset in (0..backing.len()).step_by(4096) {
        let (page, position) = backing.page(offset);
        let page = if page.len() == 4096 {
            page
        } else {
            let mut bytes = [0; 4096];
            let used = (backing.len() - offset).min(bytes.len());
            assert_eq!(page.read(position, &mut bytes[..used]), used);
            let mut copy = Page::new(4096);
            assert_eq!(copy.write(0, &bytes), bytes.len());
            copy
        };
        assert!(
            function
                .inner_mut()
                .arca_mut()
                .unwrap()
                .mappings_mut()
                .map(
                    address + offset,
                    if writable {
                        Entry::RWPage(page)
                    } else {
                        Entry::ROPage(page)
                    }
                )
                .is_ok()
        );
    }
}

pub(super) fn backing(function: &mut Function, address: usize, len: usize) -> ImmutableBytes {
    assert!(address.is_multiple_of(1 << 30));
    assert!(len <= 1 << 32 && len.is_multiple_of(HANDLE_SIZE));
    if len == 0 {
        return ImmutableBytes::new(&[]);
    }
    let mappings = function.inner_mut().arca_mut().unwrap().mappings();
    let mut tables = Vec::new();
    for offset in (0..len).step_by(1 << 30) {
        let entry = mappings
            .get((address + offset) / (1 << 30))
            .expect("handle mapping");
        let (Entry::ROTable(table) | Entry::RWTable(table)) = entry else {
            panic!("invalid handle region")
        };
        tables.push(table);
    }
    ImmutableBytes::from_tables(tables, len)
}
