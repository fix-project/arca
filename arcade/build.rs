use anyhow::Result;

fn main() -> Result<()> {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rerun-if-changed={dir}/etc/memmap.ld");
    println!("cargo::rustc-link-arg=-T{dir}/etc/memmap.ld");
    println!("cargo::rustc-link-arg=-no-pie");

    Ok(())
}
