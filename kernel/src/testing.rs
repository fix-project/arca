pub fn test_runner(tests: &[&dyn Fn()]) {
    log::info!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
}

#[no_mangle]
extern "C" fn kmain() -> ! {
    if crate::cpuinfo::is_bootstrap() {
        crate::test_main();
        unsafe { crate::shutdown() }
    }
    crate::halt();
}
