use common::{
    progress::{self, Progress, Stderr},
    protocol::control::{
        EvaluationOperation, EvaluationOutcome, EvaluationPhase, EvaluationSnapshot,
        EvaluationStep, EvaluationTask, MachineProgress, RuntimeStats,
    },
};
use std::io::Write;

fn main() {
    progress::enable();
    progress::summarize(true);
    let evaluation = Progress::advised("Evaluate named batch");
    for (location, elapsed_ns, completed) in [
        ("relay!large", 20, 2),
        ("large", 30, 3),
        ("relay!large", 20, 2),
    ] {
        progress::evaluation_progress(EvaluationSnapshot {
            machines: vec![MachineProgress {
                node: Some([1; 16]),
                location: vec![location.into()],
                used: true,
                runtime: Some(RuntimeStats {
                    elapsed_ns,
                    completed,
                    workers: 4,
                    peak_busy_workers: 2,
                    ..RuntimeStats::default()
                }),
                ..MachineProgress::default()
            }],
            steps: Vec::new(),
        });
    }
    let task = EvaluationTask {
        id: [9; 32],
        title: "transferred input".into(),
        operation: Some(EvaluationOperation::Load),
        transfer: Some((vec!["other-route".into(), "large".into()], Vec::new())),
        transfer_peer: Some([1; 16]),
        ..EvaluationTask::default()
    };
    progress::evaluation_step(EvaluationStep {
        node: None,
        sequence: 0,
        location: Vec::new(),
        task: task.clone(),
        phase: EvaluationPhase::Started,
    });
    let mut diagnostic = Stderr::default();
    write!(diagnostic, "unnamed diagnostic\n").unwrap();
    progress::evaluation_step(EvaluationStep {
        node: None,
        sequence: 1,
        location: Vec::new(),
        task,
        phase: EvaluationPhase::Finished {
            elapsed_ns: 500_000_000,
            outcome: EvaluationOutcome::Completed,
            bytes: Some(1 << 20),
        },
    });
    evaluation.finish();
    println!("result");
}
