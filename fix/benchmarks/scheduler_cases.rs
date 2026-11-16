#![no_std]

benchmark_protocol::benchmark_cases! {
    EvalEight => ("fix/scheduler/eval_eight_1worker", false, 200, 1),
    EvalWideOne => ("fix/scheduler/eval_64_1worker", false, 200, 1),
    EvalWideTwo => ("fix/scheduler/eval_64_2workers", false, 200, 2),
    EvalWideFour => ("fix/scheduler/eval_64_4workers", false, 200, 4),
    EvalWideEight => ("fix/scheduler/eval_64_8workers", false, 200, 8),
    EvalWideSixteen => ("fix/scheduler/eval_64_16workers", false, 200, 16),
}
