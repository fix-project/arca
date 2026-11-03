use runfiles::Runfiles;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    let files = Runfiles::create().unwrap();
    Command::new(files.rlocation(env!("GUEST_HARNESS_PROBE")).unwrap())
        .args(args)
        .env_remove("TESTBRIDGE_TEST_ONLY")
        .output()
        .unwrap()
}

#[test]
fn guest_tests_are_named_filtered_and_isolated() {
    let listed = run(&["--list"]);
    assert!(listed.status.success());
    let names = String::from_utf8(listed.stdout).unwrap();
    for name in ["a_panics", "b_succeeds", "c_exits_before_completion"] {
        assert!(names.contains(&format!("harness_probe::{name}: test")));
    }

    let all = run(&["--test-threads=1", "--color=never"]);
    assert!(!all.status.success());
    let output = String::from_utf8(all.stdout).unwrap();
    assert!(
        output.contains("harness_probe::a_panics ... FAILED"),
        "{output}"
    );
    assert!(
        output.contains("harness_probe::b_succeeds ... ok"),
        "{output}"
    );
    assert!(
        output.contains("harness_probe::c_exits_before_completion ... FAILED"),
        "{output}"
    );
    assert!(output.contains("guest panic probe"), "{output}");
    assert!(
        output.contains("guest exited without completing the test"),
        "{output}"
    );

    let selected = run(&["harness_probe::b_succeeds", "--exact"]);
    assert!(selected.status.success());
    let output = String::from_utf8(selected.stdout).unwrap();
    assert!(output.contains("1 passed; 0 failed"), "{output}");
    assert!(output.contains("2 filtered out"), "{output}");
}
