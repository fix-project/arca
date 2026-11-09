use std::io::{Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, SamplingMode};
use runfiles::Runfiles;

use benchmark_cases::Operation;
use benchmark_protocol::{READY, REQUEST_LEN};

struct Worker {
    child: Child,
    input: Option<ChildStdin>,
    output: ChildStdout,
}

impl Worker {
    fn start() -> Self {
        let files = Runfiles::create().unwrap();
        let vmm = files.rlocation(env!("BENCH_VMM")).unwrap();
        let image = files.rlocation(env!("BENCH_WORKER")).unwrap();
        let mut command = Command::new(vmm);
        command.arg(image).args(["--smp", "1", "--"]);
        for entry in env!("BENCH_FIXTURES").lines() {
            let (name, path) = entry.split_once('=').unwrap();
            command
                .arg("--fixture")
                .arg(name)
                .arg(files.rlocation(path).unwrap());
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let mut output = child.stdout.take().unwrap();
        let mut ready = [0; READY.len()];
        output.read_exact(&mut ready).unwrap();
        assert_eq!(ready, READY);
        Self {
            child,
            input,
            output,
        }
    }

    fn run(&mut self, operation: Operation, size: usize, iterations: u64) -> Duration {
        let mut request = [0; REQUEST_LEN];
        request[0] = operation as u8;
        request[1..9].copy_from_slice(&(size as u64).to_le_bytes());
        request[9..17].copy_from_slice(&iterations.to_le_bytes());
        self.input.as_mut().unwrap().write_all(&request).unwrap();
        let mut nanos = [0; 8];
        self.output.read_exact(&mut nanos).unwrap();
        Duration::from_nanos(u64::from_le_bytes(nanos))
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        drop(self.input.take());
        assert!(self.child.wait().unwrap().success());
    }
}

fn benchmarks(criterion: &mut Criterion) {
    let mut worker = None;
    let measurement_override = std::env::args()
        .any(|arg| arg == "--measurement-time" || arg.starts_with("--measurement-time="));
    for &operation in Operation::ALL {
        let (namespace, name) = operation.name().rsplit_once('/').unwrap();
        let mut group = criterion.benchmark_group(if operation.varies_size() {
            operation.name()
        } else {
            namespace
        });
        group.sampling_mode(SamplingMode::Flat);
        if !measurement_override {
            group.measurement_time(operation.measurement_time());
        }
        if operation.varies_size() {
            for size in [0, 65536, 1048576] {
                let mut prepared = false;
                group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, size| {
                    let worker = worker.get_or_insert_with(Worker::start);
                    if !prepared {
                        assert_eq!(worker.run(operation, *size, 0), Duration::ZERO);
                        prepared = true;
                    }
                    b.iter_custom(|iterations| worker.run(operation, *size, iterations));
                });
            }
        } else {
            let mut prepared = false;
            group.bench_function(name, |b| {
                let worker = worker.get_or_insert_with(Worker::start);
                if !prepared {
                    assert_eq!(worker.run(operation, 0, 0), Duration::ZERO);
                    prepared = true;
                }
                b.iter_custom(|iterations| worker.run(operation, 0, iterations));
            });
        }
        group.finish();
    }
}

fn main() {
    assert!(
        std::env::args().any(|arg| arg == "--discard-baseline"),
        "run with --discard-baseline; benchmark results are terminal-only"
    );
    let mut criterion = Criterion::default()
        .without_plots()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(50))
        .measurement_time(Duration::from_millis(50))
        .nresamples(10000)
        .configure_from_args();
    benchmarks(&mut criterion);
    criterion.final_summary();
}
