use crate::host::{fs, os};
use crate::kthread::KMutex;
use crate::prelude::*;
use alloc::collections::BTreeMap;
use common::protocol::control::StandardStream;

pub struct TestCase {
    pub name: &'static str,
    pub run: fn(),
}

static FIXTURES: KMutex<BTreeMap<String, &'static [u8]>> = KMutex::new(BTreeMap::new());

pub fn fixture(name: &str) -> &'static [u8] {
    let mut fixtures = FIXTURES.lock();
    if let Some(bytes) = fixtures.get(name) {
        return bytes;
    }
    let args = os::argv();
    let path = args[1..]
        .as_chunks::<3>()
        .0
        .iter()
        .find(|entry| entry[0] == "--fixture" && entry[1] == name)
        .unwrap_or_else(|| panic!("missing test fixture {name}"))[2]
        .clone();
    let bytes = Box::leak(
        fs::read(&path)
            .expect("read test fixture")
            .into_boxed_slice(),
    );
    fixtures.insert(name.into(), bytes);
    bytes
}

pub fn harness(tests: &[&TestCase]) {
    assert!(!tests.is_empty(), "guest test image contains no tests");
    let args = os::argv();
    let mut output = fs::File::stdio(StandardStream::Stdout).unwrap();
    let mode = args.last().expect("missing guest test command");
    if mode == "--list" {
        for test in tests {
            output.try_write_all(test.name.as_bytes()).unwrap();
            output.try_write_all(b"\n").unwrap();
        }
    } else {
        let name = mode
            .strip_prefix("--case=")
            .expect("invalid guest test command");
        let test = tests
            .iter()
            .find(|test| test.name == name)
            .expect("unknown guest test");
        (test.run)();
        output.try_write_all(b"\0ARCA_TEST_COMPLETE\0").unwrap();
    }
    crate::shutdown();
}

#[cfg(test)]
#[kmain]
fn main() {
    super::test_main();
}
