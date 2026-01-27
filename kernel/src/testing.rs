#![allow(dead_code)]

pub(crate) fn harness(tests: &[&dyn Fn()]) {
    log::info!("got {} tests", tests.len());
    for t in tests {
        (*t)();
    }
}

#[kmain]
async fn main(_: &[usize]) {
    super::test_main();
}
