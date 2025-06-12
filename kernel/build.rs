use anyhow::Result;

fn main() -> Result<()> {
    cc::Build::new()
        .file("src/interrupts.S")
        .compile("interrupts");
    println!("cargo::rerun-if-changed=src/interrupts.S");
    cc::Build::new().file("src/util.S").compile("util");
    println!("cargo::rerun-if-changed=src/util.S");

    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rerun-if-changed=etc/memmap.ld");
    println!("cargo::rustc-link-arg=-T{dir}/etc/memmap.ld");
    println!("cargo::rustc-link-arg=-no-pie");
    Ok(())
}
