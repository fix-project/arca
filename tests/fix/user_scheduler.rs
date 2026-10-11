#![no_std]
#![no_main]

extern crate user;
use user::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let combination: Blob = os::argument().try_into().unwrap();
    let tree: Table = Function::symbolic("get_tree")
        .apply(combination)
        .call_with_current_continuation()
        .try_into()
        .unwrap();
    let mut root = [0; 32];
    read_backing(tree, 32, &mut root);
    let input: Table = Function::symbolic("get_blob")
        .apply(Blob::new(root))
        .call_with_current_continuation()
        .try_into()
        .unwrap();
    let result: Blob = Function::symbolic("create_tree")
        .apply(Blob::new(root))
        .call_with_current_continuation()
        .try_into()
        .unwrap();
    let mut bytes = [0; 64];
    read_backing(input, 0, &mut bytes);
    assert_eq!(bytes, [42; 64]);
    Function::symbolic("scheduler_finish")
        .apply(result)
        .call_with_current_continuation();
    unreachable!()
}

fn read_backing(mut table: Table, mut offset: usize, bytes: &mut [u8]) {
    loop {
        let span = table.len() / 512;
        match table.get(offset / span).unwrap() {
            Entry::ROTable(child) => {
                table = child;
                offset %= span;
            }
            Entry::ROPage(page) => {
                assert_eq!(page.read(offset % span, bytes), bytes.len());
                return;
            }
            _ => panic!("expected immutable page backing"),
        }
    }
}
