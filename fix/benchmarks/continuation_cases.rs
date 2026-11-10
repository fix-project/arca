#![no_std]

benchmark_protocol::benchmark_cases! {
    FixKernelCaptureResume => ("fix/continuation/kernel_capture_resume", true, 50),
    FixKernelDescriptorCaptureResume => ("fix/continuation/kernel_descriptor_capture_resume", true, 50),
    FixCaptureResume => ("fix/continuation/capture_resume", true, 50),
    FixFunctionEffectRoundTrip => ("fix/function_effect_round_trip", true, 200),
    FixCanonicalize => ("fix/continuation/canonicalize", true, 1000),
    FixReplay => ("fix/continuation/replay", true, 500),
}
