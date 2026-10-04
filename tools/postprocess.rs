mod postprocessor;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let input = args.next().ok_or_else(|| {
        anyhow::anyhow!("postprocess <input.wasm> <output.wasm> [--memories N] [--tables N]")
    })?;
    let output = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing output path"))?;
    let mut memories = None;
    let mut tables = None;
    while let Some(option) = args.next() {
        let count: u32 = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("missing resource count"))?
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("invalid resource count"))?
            .parse()?;
        let target = match option.to_str() {
            Some("--memories") => &mut memories,
            Some("--tables") => &mut tables,
            _ => anyhow::bail!("unknown option {option:?}"),
        };
        anyhow::ensure!(
            target.replace(count).is_none(),
            "duplicate option {option:?}"
        );
    }
    std::fs::write(
        output,
        postprocessor::process(&std::fs::read(input)?, memories, tables)?,
    )?;
    Ok(())
}
