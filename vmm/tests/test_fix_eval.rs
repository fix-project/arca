use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const EXPECTED_OUTPUT: &str = "as a u64: 6";
// Time limited so this test doesn't stall the test suite (usually runs in <10s)
const DEADLOCK_LIMIT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

// Fail instead of hanging if the file-close handshake deadlocks
fn wait_for_output(mut child: Child) -> Output {
    let start = Instant::now();

    loop {
        match child.try_wait().expect("poll Fix-on-Arca process") {
            Some(_) => {
                // Collect both captured streams after the child exits.
                return child
                    .wait_with_output()
                    .expect("collect Fix-on-Arca output");
            }
            None if start.elapsed() < DEADLOCK_LIMIT => thread::sleep(POLL_INTERVAL),
            None => {
                // Stop the hung VM before reporting diagnostics
                let _ = child.kill();
                let output = child
                    .wait_with_output()
                    .expect("collect timed-out Fix-on-Arca output");
                panic!(
                    "fix eval did not complete file-close handshake\nstdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr),
                );
            }
        }
    }
}

#[test]
fn fix_eval_completes_and_prints_result() {
    // Use the .fix program at project root
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("vmm crate is inside the workspace");
    let program = workspace.join("addblob.fix");
    assert!(program.is_file(), "missing Fix test program: {program:?}");

    // Run `cargo build -p fix --target=x86_64-unknown-none` before this test.
    // The guest binary should be at "target/x86_64-unknown-none/debug/fix".
    let child = Command::new(env!("CARGO_BIN_EXE_vmm"))
        .arg(workspace.join("target/x86_64-unknown-none/debug/fix"))
        .arg("eval")
        .arg(&program)
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch Fix-on-Arca under the VMM");

    let output = wait_for_output(child);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Successful status means guest reached kernel shutdown
    assert!(
        output.status.success(),
        "fix eval failed with {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status,
    );
    // Show stderr if output is not as expected
    assert!(
        stderr.contains(EXPECTED_OUTPUT),
        "fix eval did not print the expected result\nstdout:\n{stdout}\nstderr:\n{stderr}",
    );
}
