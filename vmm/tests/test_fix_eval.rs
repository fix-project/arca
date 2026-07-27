use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

// Keep the regression input with the test instead of relying on a fixture
const FIX_PROGRAM: &str = r#"(let ((add @"./target/x86_64-unknown-none/addblob"))
    !*(add !*(add 2 3) 1))
"#;
const EXPECTED_OUTPUT: &str = "as a u64: 6";
// Time limited so this test doesn't stall the test suite (usually runs in <10s)
const DEADLOCK_LIMIT: Duration = Duration::from_secs(30);
// Poll lightly instead of busy-waiting for the child
const POLL_INTERVAL: Duration = Duration::from_millis(10);

struct ProgramFile {
    path: PathBuf,
}

impl ProgramFile {
    fn new() -> Self {
        // The production CLI requires a path, so materialize the inline program
        let path = std::env::temp_dir().join(format!("arca-fix-eval-{}.fix", std::process::id()));
        std::fs::write(&path, FIX_PROGRAM).expect("write temporary Fix program");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ProgramFile {
    fn drop(&mut self) {
        // Clean up the temp file
        let _ = std::fs::remove_file(&self.path);
    }
}

// Fail instead of hanging if the file-close handshake deadlocks
fn wait_for_output(mut child: Child) -> Output {
    let start = Instant::now();

    loop {
        match child.try_wait().expect("poll Fix-on-Arca process") {
            Some(_) => {
                // Kill the child and collect both captured streams
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
                    "fix eval did not complete its file-close handshake\nstdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr),
                );
            }
        }
    }
}

#[test]
fn fix_eval_completes_and_prints_result() {
    let program = ProgramFile::new();
    // Resolve the program's addblob helper from the workspace target directory
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("vmm crate is inside the workspace");

    // Cargo supplies both binaries; VMM expects the guest ELF before guest argv
    let child = Command::new(env!("CARGO_BIN_EXE_vmm"))
        .arg(env!("CARGO_BIN_FILE_FIX_GUEST_fix"))
        .arg("eval")
        .arg(program.path())
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch Fix-on-Arca under the VMM");

    // Decode captured bytes for assertions and failure diagnostics
    let output = wait_for_output(child);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // A successful status proves the guest reached kernel shutdown
    assert!(
        output.status.success(),
        "fix eval failed with {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status,
    );
    // VMM forwards guest debug-console output to stderr
    assert!(
        stderr.contains(EXPECTED_OUTPUT),
        "fix eval did not print the expected result\nstdout:\n{stdout}\nstderr:\n{stderr}",
    );
}
