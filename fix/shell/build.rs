use anyhow::Result;

fn main() -> Result<()> {
    println!("cargo::rerun-if-changed=etc/memmap.ld");
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rustc-link-arg=-T{dir}/etc/memmap.ld");
    println!("cargo::rustc-link-arg=-no-pie");
    Ok(())
}
