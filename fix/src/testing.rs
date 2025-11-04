pub fn test_runner(tests: &[&dyn Fn()]) {
    for test in tests {
        test();
    }
}
