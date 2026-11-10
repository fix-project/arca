#![no_std]

benchmark_protocol::benchmark_cases! {
    Metadata => ("fix/sdk/metadata", false, 100),
    HandleRead => ("fix/sdk/owned_read_u64", false, 100),
    Read => ("fix/sdk/read_u64", false, 100),
    Tree => ("fix/sdk/create_tree", false, 100),
    CloneClose => ("fix/sdk/clone_close", false, 100),
    EvalValue => ("fix/sdk/eval_value", false, 200),
    EvalIdentification => ("fix/sdk/eval_identification", false, 200),
    EvalApplication => ("fix/sdk/eval_application", false, 200),
}
