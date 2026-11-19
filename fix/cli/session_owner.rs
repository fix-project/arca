fn main() -> std::process::ExitCode {
    let kernel = std::sync::Arc::from(&include_bytes!(env!("FIX_KERNEL"))[..]);
    match vmm::service::run_owner(kernel) {
        Ok(status) => std::process::ExitCode::from(status),
        Err(error) => {
            eprintln!("fix session: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
