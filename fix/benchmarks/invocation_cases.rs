#![no_std]

benchmark_protocol::benchmark_cases! {
    FixInvoke => ("fix/invoke", false, 100),
    FixInvokeCertified => ("fix/invoke_certified", false, 100),
    FixChain => ("fix/chain_16", false, 500),
}
