fn main() {
    cc::Build::new().file("src/start.S").compile("start");
    println!("cargo::rerun-if-changed=src/start.S");

    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo::rerun-if-changed=etc/memmap.ld");
    println!("cargo::rustc-link-arg=-T{}/etc/memmap.ld", dir);
    println!("cargo::rustc-link-arg=-no-dynamic-linker");
    println!("cargo::rustc-link-arg=-no-pie");
}
