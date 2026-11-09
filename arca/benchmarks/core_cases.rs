#![no_std]

benchmark_protocol::benchmark_cases! {
    NativeCall => ("native/call", false, 50),
    ArcaCreateDestroy => ("arca/create_destroy", false, 50),
    ArcaElfCreateDestroy => ("arca/elf_create_destroy", false, 50),
    ArcaInvoke => ("arca/invoke", false, 50),
}
