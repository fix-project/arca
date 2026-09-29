mod postprocessor;

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    anyhow::ensure!(args.len() == 3, "postprocess <input.wasm> <output.wasm>");
    std::fs::write(&args[2], postprocessor::process(&std::fs::read(&args[1])?)?)?;
    Ok(())
}
