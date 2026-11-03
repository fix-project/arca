#![feature(test)]

extern crate test;

use runfiles::Runfiles;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Output};
use std::sync::Arc;

struct Guest {
    vmm: PathBuf,
    image: PathBuf,
    fixtures: Vec<(String, PathBuf)>,
}

impl Guest {
    fn new() -> Self {
        let files = Runfiles::create().unwrap();
        let locate = |name: &str| files.rlocation(name).unwrap();
        Self {
            vmm: locate(env!("GUEST_TEST_VMM")),
            image: locate(env!("GUEST_TEST_IMAGE")),
            fixtures: env!("GUEST_TEST_FIXTURES")
                .lines()
                .map(|entry| {
                    let (name, path) = entry.split_once('=').unwrap();
                    (name.into(), locate(path))
                })
                .collect(),
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.vmm);
        command.arg(&self.image).args(["--smp", "2", "--"]);
        for (name, path) in &self.fixtures {
            command.arg("--fixture").arg(name).arg(path);
        }
        command
    }

    fn list(&self) -> Vec<String> {
        let output = self.command().arg("--list").output().unwrap();
        assert!(
            output.status.success(),
            "guest discovery failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let names: Vec<_> = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
        assert!(!names.is_empty(), "guest image contains no tests");
        names
    }

    fn run(&self, name: &str, index: usize) {
        let directory =
            PathBuf::from(std::env::var_os("TEST_TMPDIR").unwrap()).join(index.to_string());
        std::fs::create_dir_all(&directory).expect("create guest test directory");
        let Output {
            status,
            stdout,
            stderr,
        } = self
            .command()
            .arg(format!("--case={name}"))
            .current_dir(directory)
            .output()
            .expect("run guest test");
        let completed = stdout.strip_suffix(b"\0ARCA_TEST_COMPLETE\0");
        print!("{}", String::from_utf8_lossy(completed.unwrap_or(&stdout)));
        eprint!("{}", String::from_utf8_lossy(&stderr));
        assert!(status.success(), "guest exited with {status}");
        assert!(
            completed.is_some(),
            "guest exited without completing the test"
        );
    }
}

fn main() -> ExitCode {
    let guest = Arc::new(Guest::new());
    let cases: Vec<_> = guest
        .list()
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let guest = guest.clone();
            let selected = name.clone();
            test::TestDescAndFn {
                desc: test::TestDesc {
                    name: test::DynTestName(name),
                    ignore: false,
                    ignore_message: None,
                    source_file: "<guest>",
                    start_line: 0,
                    start_col: 0,
                    end_line: 0,
                    end_col: 0,
                    should_panic: test::ShouldPanic::No,
                    compile_fail: false,
                    no_run: false,
                    test_type: test::TestType::UnitTest,
                },
                testfn: test::DynTestFn(Arc::new(move || {
                    guest.run(&selected, index);
                    Ok(())
                })),
            }
        })
        .collect();
    let mut args: Vec<_> = std::env::args().collect();
    if let Ok(filter) = std::env::var("TESTBRIDGE_TEST_ONLY") {
        if !filter.is_empty() {
            args.push(filter);
        }
    }
    test::test_main(&args, &cases.iter().collect::<Vec<_>>())
}
