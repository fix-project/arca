#![no_std]

benchmark_protocol::benchmark_cases! {
    ArcaEffectRoundTrip => ("arca/effect_round_trip", true, 50),
    ArcaFunctionEffectRoundTrip => ("arca/function_effect_round_trip", true, 50),
    ArcaClone => ("arca/continuation/clone", true, 50),
    ArcaCopyResumeDirty => ("arca/continuation/copy_resume_dirty", true, 50),
}
