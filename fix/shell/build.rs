use anyhow::Result;

fn main() -> Result<()> {
    println!("cargo::rustc-link-arg=-no-pie");
    Ok(())
}
