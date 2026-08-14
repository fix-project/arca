use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const EXPECTED_OUTPUT: &str = "as a u64: 6";
const FIX_GUEST: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_FIX_GUEST_fix"));
const FIX_PROGRAM: &str = r#"(let ((add @"{addblob}"))
    !*(add !*(add 2 3) 1))
"#;
const GUEST_PROCESS_ENV: &str = "ARCA_FIX_EVAL_GUEST_PROCESS";
// Time limited so this test doesn't stall the test suite (usually runs in <10s)
const DEADLOCK_LIMIT: Duration = Duration::from_secs(30);
// Poll lightly instead of busy-waiting for the child
const POLL_INTERVAL: Duration = Duration::from_millis(10);

struct ProgramFile {
    path: PathBuf,
    addblob_path: PathBuf,
}

impl ProgramFile {
    fn new() -> Self {
        // The CLI requires paths, so materialize the embedded helper and inline program
        let addblob_path =
            std::env::temp_dir().join(format!("arca-fix-eval-addblob-{}", std::process::id()));
        let addblob = fix_wasm::artifact("addblob").expect("find embedded addblob guest");
        std::fs::write(&addblob_path, addblob).expect("write temporary addblob guest");

        let path = std::env::temp_dir().join(format!("arca-fix-eval-{}.fix", std::process::id()));
        let program = FIX_PROGRAM.replace("{addblob}", addblob_path.to_string_lossy().as_ref());
        std::fs::write(&path, program).expect("write temporary Fix program");
        Self { path, addblob_path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ProgramFile {
    fn drop(&mut self) {
        // Clean up the temp file
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(&self.addblob_path);
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
    if std::env::var_os(GUEST_PROCESS_ENV).is_some() {
        let program = ProgramFile::new();
        let mut runtime = vmm::runtime::Runtime::new(1, 1 << 34, FIX_GUEST.into());
        runtime.run(vec![
            "fix".to_owned(),
            "eval".to_owned(),
            program.path().to_string_lossy().into_owned(),
        ]);
        return;
    }

    // Run from the workspace so relative Fix paths match normal CLI use.
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("vmm crate is inside the workspace");

    // Re-run this test binary so the VM can be timed out and its output captured.
    let child = Command::new(std::env::current_exe().expect("locate Fix-on-Arca test binary"))
        .arg("--exact")
        .arg("fix_eval_completes_and_prints_result")
        .arg("--nocapture")
        .env(GUEST_PROCESS_ENV, "1")
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
