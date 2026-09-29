#![allow(dead_code)]

pub(crate) fn harness(tests: &[&dyn Fn()]) {
    assert!(!tests.is_empty(), "kernel test image contains no tests");
    log::info!("got {} tests", tests.len());
    for t in tests {
        (*t)();
    }
}

#[kmain]
fn main() {
    super::test_main();
}
