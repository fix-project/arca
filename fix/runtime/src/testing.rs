pub fn test_runner(tests: &[&dyn Fn()]) {
    assert!(!tests.is_empty(), "Fix test image contains no tests");
    kernel::println!("running {} Fix tests", tests.len());
    for test in tests {
        test();
    }
}
