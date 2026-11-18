use common::{
    progress::{self, Progress, Stderr},
    protocol::control::{
        EvaluationOperation, EvaluationSnapshot, EvaluationStep, EvaluationTask, MachineProgress,
        RemoteAvailability, RuntimeStats,
    },
};
use std::{io::Write, time::Duration};

fn main() {
    progress::enable();
    progress::summarize(std::env::args().any(|arg| arg == "--summarize"));
    let evaluation = Progress::advised("Evaluate named batch");
    let load = EvaluationTask {
        id: [2; 32],
        operation: Some(EvaluationOperation::Load),
        transfer: Some((vec!["origin".into()], Vec::new())),
        ..EvaluationTask::default()
    };
    progress::evaluation_progress(EvaluationSnapshot {
        machines: vec![
            MachineProgress {
                runtime: Some(RuntimeStats {
                    workers: 4,
                    memory_used: 128 << 20,
                    memory_total: 512 << 20,
                    peak_memory: 128 << 20,
                    cpu_ns: Some(0),
                    ..RuntimeStats::default()
                }),
                used: true,
                tasks: vec![load.clone()],
                labels: vec![
                    ([2; 32], "named input".into()),
                    ([6; 32], "named result".into()),
                ],
                ..MachineProgress::default()
            },
            MachineProgress {
                location: vec!["origin".into()],
                availability: Some(RemoteAvailability::Available {
                    workers: 4,
                    memory_bytes: Some(512 << 20),
                }),
                ..MachineProgress::default()
            },
        ],
        steps: Vec::new(),
    });
    evaluation.message("ephemeral coordinator progress");
    let activity = Progress::activity(format!("ephemeral activity {}", "long label ".repeat(20)));
    activity.message("work in progress");
    progress::advice(
        [1; 32],
        Some(EvaluationOperation::Apply),
        b"fix::title",
        b"named computation",
    );
    let mut diagnostic = Stderr::default();
    diagnostic.write_all(b"diagnostic ").unwrap();
    std::thread::sleep(Duration::from_millis(250));
    diagnostic.write_all(b"preserved\n").unwrap();
    progress::evaluation_step(EvaluationStep {
        location: Vec::new(),
        task: load,
        elapsed_ns: 250_000_000,
        success: true,
    });
    progress::evaluation_step(EvaluationStep {
        location: Vec::new(),
        task: EvaluationTask {
            id: [1; 32],
            operation: Some(EvaluationOperation::Apply),
            title: "named computation".into(),
            ..EvaluationTask::default()
        },
        elapsed_ns: 1_000_000,
        success: true,
    });
    progress::evaluation_step(EvaluationStep {
        location: Vec::new(),
        task: EvaluationTask {
            id: [3; 32],
            title: "generated result".into(),
            operation: Some(EvaluationOperation::Load),
            transfer: Some((Vec::new(), vec!["origin".into()])),
            ..EvaluationTask::default()
        },
        elapsed_ns: 1_000_000,
        success: false,
    });
    progress::evaluation_step(EvaluationStep {
        location: Vec::new(),
        task: EvaluationTask {
            id: [4; 32],
            title: "resident input".into(),
            operation: Some(EvaluationOperation::Load),
            transfer: Some((Vec::new(), Vec::new())),
            ..EvaluationTask::default()
        },
        elapsed_ns: 1_000_000,
        success: true,
    });
    progress::evaluation_step(EvaluationStep {
        location: Vec::new(),
        task: EvaluationTask {
            id: [7; 32],
            operation: Some(EvaluationOperation::Load),
            transfer: Some((vec!["origin".into()], Vec::new())),
            name: "Eval(named input, named result)".into(),
            ..EvaluationTask::default()
        },
        elapsed_ns: 1_000_000,
        success: true,
    });
    progress::suspend(|| println!("result preserved"));
    drop(activity);
    if std::env::args().any(|arg| arg == "complete") {
        evaluation.finish();
    } else {
        drop(evaluation);
    }
    diagnostic.write_all(b"final diagnostic").unwrap();
    diagnostic.flush().unwrap();
    std::thread::sleep(Duration::from_millis(250));
    progress::println("finished");
}
